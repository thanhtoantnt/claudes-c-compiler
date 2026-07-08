# Bug Report: `encode_cinc` emits a reserved/UNPREDICTABLE CSINC for `al` / `nv`

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_cinc`

## Summary

`CINC` is an alias of `CSINC`: `CINC Rd, Rn, cond` encodes as
`CSINC Rd, Rn, Rn, invert(cond)`, where the condition's least-significant bit
is flipped. The ARM ARM (C4.1.66, "Conditional select") constrains the `cond`
field of CSINC to **not** be `1110` (`al`) or `1111` (`nv`) — those encodings
are reserved / UNPREDICTABLE, and reference assemblers (GNU `as`, LLVM
`llvm-mc`) reject them.

Because `al` (14) and `nv` (15) are bitwise complements in the low bit,
inverting maps `al` -> `nv` and `nv` -> `al`. `encode_cinc` performs no check
on the inverted condition, so `cinc Rd, Rn, al` / `cinc Rd, Rn, nv` are
accepted and emit a CSINC whose `cond` field is the reserved `nv` / `al`
encoding.

This is the same defect already characterized for the sibling condition-
inverting alias `encode_csetm` (-> CSINV) in this same file, but `encode_cinc`
is affected independently and needs its own fix.

## Reproduction

The negative-contract property `prop_encode_cinc_tests::prop_rejects_invalid_operands`
enumerates the reserved conditions at cases 16-19; proptest shrinks the
*combined* property to the FP/SIMD case first (see companion report
`encode_cinc_silently_accepts_fp_simd_registers.md`), so the `al`/`nv` defect
is demonstrated here by the **passing** structural/algebraic properties that
range over the full condition table (including `al`/`nv`):

- `prop_opcode_structure_and_fields` (PASS) asserts, for every entry in
  `COND_TABLE`, that `encode_cinc` returns `Ok` with
  `(word >> 12) & 0xF == cond_val ^ 1`. For `cond = "al"` (14) it therefore
  proves the emitted `cond` field is `15` (`nv`); for `cond = "nv"` (15) it
  proves the emitted field is `14` (`al`).

Minimal inputs and the words they produce (field-reconstructed from the passing
oracle; exact values verifiable by running `encode_cinc` directly):

```
encode_cinc([Reg("x0"), Reg("x1"), Cond("al")]) -> Ok(Word(0x9A81F420))
    0x9A81F420 == csinc x0, x1, x1, nv        // cond field = 1111 (reserved)

encode_cinc([Reg("x0"), Reg("x1"), Cond("nv")]) -> Ok(Word(0x9A817420))
    0x9A817420 == csinc x0, x1, x1, al        // cond field = 1110 (reserved)
```

Root cause:

```rust
pub(crate) fn encode_cinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    ...
    let cond = match operands.get(2) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or_else(...)?,
        _ => return Err(...),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1;          // <-- al<->nv swap, never validated
    let word = (sf << 31) | (0b011010100 << 21) | (rn << 16)
        | (inv_cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))      // <-- emits reserved encoding
}
```

## Impact

Emits an architecturally reserved / UNPREDICTABLE encoding. Hardware behaviour
for CSINC with `cond` in `{al, nv}` is UNPREDICTABLE (ARM ARM C4.1.66); such
instructions must not be assembled. An assembler that accepts
`cinc Rd, Rn, al` produces object code that GAS and `llvm-mc` would have
rejected, silently degrading portability and correctness.

## Suggested fix

Reject `al` / `nv` before emitting (their inverses are reserved). Equivalently,
reject when the *inverted* condition value is `>= 14`:

```rust
pub(crate) fn encode_cinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    ...
    let cond = match operands.get(2) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or_else(...)?,
        _ => return Err(...),
    };
    let inv_cond = cond ^ 1;
    // CSINC cond must not be al(14) or nv(15): UNPREDICTABLE (ARM ARM C4.1.66).
    // Since CINC inverts the condition, reject al/nv inputs (which map to nv/al).
    if inv_cond >= 14 {
        return Err(format!(
            "cinc: condition yields reserved aliased CSINC condition (al/nv)"
        ));
    }
    let sf = sf_bit(is_64);
    ...
}
```

## Test coverage added

Five `proptest!` properties added to `prop_encode_cinc_tests` in
`compare_branch.rs`:

| Property | Oracle | Result |
|---|---|---|
| `prop_opcode_structure_and_fields` | Reference (ranges over full cond table incl. al/nv; **demonstrates acceptance of reserved encodings**) | PASS |
| `prop_cinc_equals_csinc_alias` | Differential vs `encode_csinc` | PASS |
| `prop_rm_equals_rn` | Invariant (Rm field == Rn field) | PASS |
| `prop_condition_is_inverted` | Algebraic (complementary pairs differ only in bit 12; al<->nv, nv<->al) | PASS |
| `prop_rejects_invalid_operands` | Negative/error contract (this finding, cases 16-19; combined property FAILS, shrinks to FP case first) | **FAIL** |
