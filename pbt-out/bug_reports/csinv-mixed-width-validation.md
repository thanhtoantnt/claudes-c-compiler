# Bug Report: `encode_csinv` does not validate mixed register widths

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_csinv`

## Summary

`encode_csinv` silently accepts CSINV instructions whose three register operands
have *mixed* widths (e.g. `csinv w0, w0, x0, eq` — a 32-bit destination and source
mixed with a 64-bit source). The single `sf` bit in the AArch64 conditional-select
encoding governs the width of `Rd`, `Rn`, and `Rm` collectively, so any
non-uniform-width triple is architecturally UNALLOCATED and must be rejected.

## Minimal input

```
ops = [Reg("w0"), Reg("w0"), Reg("x0"), Cond("eq")]   // mnemonic: csinv w0, w0, x0, eq
```

## Expected vs actual

- **Expected:** `Err` (UNALLOCATED encoding — mixed 32-bit/64-bit register widths).
- **Actual:** `Ok(Word(0x5A800000))` (i.e. `1518338048`). The 64-bit `x0` source is
  silently re-encoded as if it were 32-bit (`sf=0` taken from `w0`); the emitted
  instruction does not match the source text.

Source: minimized counterexample from property
`prop_encode_csinv_tests::prop_rejects_mixed_width_operands`
(`rd_is64=false, rn_is64=false, rm_is64=true, rd=0, rn=0, rm=0, cond_idx=0`).

## Impact

- A malformed instruction word is emitted for any mixed-width CSINV; downstream
  linker/disassembler/simulator consumers receive a well-formed but semantically
  wrong word and cannot distinguish it from a correctly-encoded CSINV.
- The reverse case (`csinv x0, x0, w0, eq`) is likewise accepted with `sf=1`,
  silently widening the 32-bit `w0`.
- Confirmed by the passing characterization property
  `prop_rn_and_rm_widths_do_not_affect_word`: holding `Rd` fixed, flipping `Rn`/`Rm`
  x↔w width leaves the encoded word bit-identical, proving those widths are
  discarded.

## Root cause

```rust
pub(crate) fn encode_csinv(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width taken ONLY from Rd
    let (rn, _) = get_reg(operands, 1)?;        // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;        // is_64 discarded
    ...
    let sf = sf_bit(is_64);
    ...
}
```

`get_reg` (encoder/mod.rs:956) returns `(num, is_64)`, but only the `is_64` from
operand 0 (`Rd`) is used for `sf`; the widths of `Rn` and `Rm` are bound to `_`
and discarded. No consistency check compares the three widths, so a mixed-width
triple is never rejected. This is the same defect class documented for
`encode_smull` (`SMULL_BUG_REPORT.md`).

## Fix

After the three `get_reg` calls, assert the widths agree:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if rn_is_64 != is_64 || rm_is_64 != is_64 {
    return Err("csinv: all register operands must have the same width".into());
}
```
