# Bug Report: `encode_csneg` silently accepts mixed-width (X/W) register operands

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csneg`
**Severity:** High

## Summary

`encode_csneg` derives `sf` (operand-size) bit **only** from destination register `Rd` and silently ignores widths of `Rn` and `Rm`. Accepts UNPREDICTABLE mixed-width instructions (e.g. `csneg w0, w0, x0, eq`).

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;   // width taken ONLY from Rd
let (rn, _) = get_reg(operands, 1)?;        // width DISCARDED
let (rm, _) = get_reg(operands, 2)?;        // width DISCARDED
let sf = sf_bit(is_64);                      // sf == Rd's width only
```

## Reproduction

**Input:** `csneg w0, w0, x0, eq`

**Expected:** `Err` — Rd, Rn, Rm must all be the same width

**Actual:** `Ok(Word(0x5A800000))` — sf=0 32-bit instruction, but Rm=x0 is 64-bit

**Minimal failing input:** rd=0, rn=0, rm=0, rd_is64=false, rn_is64=false, rm_is64=true

## Impact

UNPREDICTABLE encodings emitted without diagnostic. Any mixed-width permutation accepted. Same gap affects `encode_csel`, `encode_csinc`, `encode_csinv`.

## Suggested Fix

Validate width agreement across all three operands:

```rust
let (rd, rd_is64) = get_reg(operands, 0)?;
let (rn, rn_is64) = get_reg(operands, 1)?;
let (rm, rm_is64) = get_reg(operands, 2)?;
if rd_is64 != rn_is64 || rd_is64 != rm_is64 {
    return Err("csneg: Rd, Rn, Rm must all be the same width".to_string());
}
let sf = sf_bit(rd_is64);
```

## Regression Property

Failing property: `prop_rejects_mixed_width_operands`

```rust
prop_assert!(encode_csneg(&[wreg(0), wreg(0), xreg(0), cond("eq")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/165