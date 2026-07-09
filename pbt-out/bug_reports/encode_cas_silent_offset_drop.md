# `encode_cas` silently drops a non-zero memory offset (`[Xn, #imm]`)

**Severity:** High (wrong instruction emitted with no diagnostic — silent miscompilation)
**Component:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_cas`
**Found by:** Property-based tests in `prop_encode_cas_offset_tests`
**Date:** 2026-07-09

## Summary

`encode_cas` matches the third operand as `Operand::Mem { base, .. }`, using `..`
to **ignore the offset field**. Per the ARMv8.1-A architecture (ARM ARM §C6.2.21,
*Compare and Swap*), every CAS variant (`cas`/`casa`/`casl`/`casal` and the
`casb*`/`cash*` byte/halfword forms) supports **only** the plain `[Xn|SP]`
addressing mode. There is no immediate-offset, pre-index, post-index, or
register-offset form — the fixed `11111` field at bits `[14:10]` of the encoding
is the literal encoding of "no offset / no index register".

As a result, `cas x0, x1, [x2, #16]` is silently accepted and encoded as
`cas x0, x1, [x2]` — the offset is discarded with no error. The assembler has
produced an instruction with different semantics than the one the programmer
wrote, and emitted no diagnostic.

## Reproduction

Property `prop_nonzero_immediate_offset_rejected` fails on its first input:

```
minimal failing input: vi = 0, offset = 1, neg = false, rn = 0
cas with [x0, #1] (nonzero offset) must be Err: CAS has no offset addressing mode.
Got Ok — offset was silently dropped, producing an unintended `cas ...,[x0]` encoding.
```

Concrete case:

```rust
encode_cas(
    "cas",
    &[
        Operand::Reg("x0".into()),
        Operand::Reg("x1".into()),
        Operand::Mem { base: "x2".into(), offset: 16 },
    ],
)
// Returns Ok(0xC8A07C42)  == encoding of `cas x0, x1, [x2]`
// Expected: Err(...)       — CAS has no offset addressing mode
```

The returned word is identical to the offset-0 form, proving the offset was
dropped rather than encoded.

## Root cause

`load_store.rs:818-821`:

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("cas: invalid base")?,
    _ => return Err("cas requires memory operand [Xn]".to_string()),
};
```

The `..` pattern binds only `base`, so any non-zero `offset` is accepted and
discarded. (Note: `MemPreIndex`, `MemPostIndex`, and `MemRegOffset` correctly
fall into the `_` arm and *are* rejected — only the `Operand::Mem` non-zero
offset case is mishandled.)

## Impact

* **Silent miscompilation.** `cas Xs, Xt, [Xn, #N]` assembles to
  `cas Xs, Xt, [Xn]`, operating on the wrong address (off by `N` bytes). For an
  atomic compare-and-swap this is a data-correctness hazard with no build-time
  signal.
* **Asymmetry.** Offset/writeback forms of every *other* memory operand are
  rejected; only the immediate-offset sub-case of `Operand::Mem` leaks through.
* Violates the project's own documented standard (see comment at
  `load_store.rs:2585`: *"emitting a wrong instruction word with no
  diagnostic"*).

## Suggested fix

Reject any non-zero offset explicitly before binding `rn`:

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, offset: 0 }) => {
        parse_reg_num(base).ok_or("cas: invalid base")?
    }
    Some(Operand::Mem { base, offset }) => {
        return Err(format!(
            "{} requires [Xn] addressing with no offset (got offset {})",
            mnemonic, offset
        ));
    }
    _ => return Err("cas requires memory operand [Xn]".to_string()),
};
```

The same pattern applies to `encode_swp` (`load_store.rs:853-856`), which uses
the identical `Operand::Mem { base, .. }` match and has the same defect.

## Properties

| Property | Result | Notes |
|---|---|---|
| `prop_zero_offset_accepted_and_invariant` | ✅ pass | offset==0 is the only valid form; explicit 0 ≡ canonical `[Xn]` |
| `prop_nonzero_immediate_offset_rejected` | ❌ **FAIL** | documents this bug |
| `prop_pre_index_writeback_rejected` | ✅ pass | `[Xn,#imm]!` correctly rejected via `_` arm |
| `prop_post_index_writeback_rejected` | ✅ pass | `[Xn],#imm` correctly rejected via `_` arm |
| `prop_register_offset_rejected` | ✅ pass | `[Xn,Xm]` correctly rejected via `_` arm |
