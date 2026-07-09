# BUG: `encode_ldxr_stxr` silently drops the immediate offset (no range validation)

## Status
Confirmed by property-based tests — 2 failing properties (negative-contract),
2 passing properties that document the drop mechanism.

## Summary
`encode_ldxr_stxr` (LDXR/STXR — Load/Store Exclusive Register) accepts **any**
immediate offset in its memory operand and silently discards it, encoding the
instruction as though the offset were `0`. Because the AArch64 LDXR/STXR
encoding has **no immediate-offset field**, a non-zero offset is unrepresentable
and must be rejected — but instead it is dropped without warning, producing an
instruction with the wrong effective address.

## Location
`src/backend/arm/assembler/encoder/load_store.rs` — `encode_ldxr_stxr`, **both**
the load branch and the store branch:

```rust
// load branch
let rn = match operands.get(1) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
    //                                         ^^^ `offset` is never inspected
    ...
};
// store branch
let rn = match operands.get(2) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
    //                                         ^^^ `offset` is never inspected
    ...
};
```

The `..` in the `Mem` pattern binds the `offset` field and immediately throws
it away. There is no `*offset != 0` check anywhere in the function.

## Spec reference (ARM ARM)
The exclusive single-register load/store group has **no offset form**. The only
permitted syntax is:

```
LDXR  <Xt>, [<Xn|SP>]          STXR  <Ws>, <Xt>, [<Xn|SP>]
```

Encoding (no imm field exists):

```
LDXR: size 001000 1 0 1 11111 0 11111 Rn Rt
STXR: size 001000 0 0 1 Rs   0 11111 Rn Rt
```

Contrast with the scaled/unscaled forms in the same file which DO encode
immediates (e.g. `imm9`, `imm12`). LDXR/STXR genuinely carry none, so the offset
is not "masked/truncated" — it is *unrepresentable*. Rejection is the correct
behavior. (For comparison, `encode_prfm` in the same file correctly validates its
offset: `if imm < 0 || imm % 8 != 0 { return Err(...) }`.)

## Impact
- **Silent semantic corruption.** `ldxr x0, [x1, #8]` assembles to the bit
  pattern for `ldxr x0, [x1]` — i.e. it loads from 8 bytes away from the
  programmer's intended address. No error, no warning.
- Worst class of assembler defect: the output is a valid instruction that does
  the wrong thing. A wrong base for `stxr` can corrupt an unrelated cache line's
  exclusivity state; a wrong `ldxr` base silently breaks a lock-free algorithm.
- No defense-in-depth: any caller (hand-built `Operand` list, future parser
  change, or intermediate IR pass) can pass a non-zero offset and get a
  silently-wrong encoding.

## Reproduction
```
cargo test --lib prop_encode_ldxr_stxr_offset_tests::
```

Result:

```
prop_offset_does_not_affect_word ... ok      <- proves offset is dropped
prop_zero_offset_accepted       ... ok      <- only legal value works
prop_nonzero_offset_rejected    ... FAILED  <- off=-1 -> Ok(Word(...))
prop_common_offsets_rejected    ... FAILED  <- off=1  -> Ok(Word(...))
```

Counterexamples (both load and store):
- `encode_ldxr_stxr(&[x0, [x1,#-1]], true,  None)` -> `Ok(Word(0xC85F7C20))` (== `[x1]`)
- `encode_ldxr_stxr(&[w0,x0,[x1,#1]], false, None)` -> `Ok(Word(0xC8007C20))` (== `[x1]`)

## Suggested fix
After extracting `base`, reject a non-zero offset in both branches:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "ldxr/stxr take no immediate offset (syntax [<Xn>]); got #{}",
            offset
        ));
    }
    parse_reg_num(base).ok_or("invalid base")?
}
```

(Use `*offset != 0`, not a `& 0x1FF` mask: there is no imm9 field here to
truncate into, so masking would be wrong.)

## Test artifacts
- Properties live in module `prop_encode_ldxr_stxr_offset_tests` in
  `src/backend/arm/assembler/encoder/load_store.rs`.
- The two negative-contract properties are intentionally *expected to fail*
  until the fix lands; they double as the regression test (they turn green once
  offset validation is added).
- proptest persisted failing seeds in
  `proptest-regressions/backend/arm/assembler/encoder/load_store.txt`.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/180
