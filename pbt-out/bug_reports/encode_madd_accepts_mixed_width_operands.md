# Bug Report: `encode_madd` does not reject mixed-width operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_madd`
**Severity:** Medium

## Summary

ARMv8 ARM requires all four operands of `MADD <Rd>,<Rn>,<Rm>,<Ra>` to share one register width (all `X` or all `W`). `encode_madd` derives `sf` **only from `Rd`**, ignoring widths of `Rn`, `Rm`, `Ra`. Mixed-width `madd x0, w1, x2, x3` accepted as 64-bit operation.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let (rm, _) = get_reg(operands, 2)?;   // width discarded
let (ra, _) = get_reg(operands, 3)?;   // width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `madd x0, w1, x2, x3`

**Expected:** `Err` — all MADD operands must have the same width

**Actual:** `Ok(Word(...))` — sf=1 from Rd, Rn's W width ignored

## Impact

Mixed-width forms accepted, encoded as operation determined solely by destination width. GNU `as` and LLVM reject mixed-width MADD operands.

## Suggested Fix

Collect and assert equality of width flags:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
let (ra, ra_is_64) = get_reg(operands, 3)?;
if rn_is_64 != is_64 || rm_is_64 != is_64 || ra_is_64 != is_64 {
    return Err("all MADD operands must have the same width".into());
}
```

## Regression Property

Failing property: `madd_rejects_mixed_width_operands`

```rust
prop_assert!(encode_madd(&[xreg(0), wreg(1), xreg(2), xreg(3)]).is_err());
prop_assert!(encode_madd(&[wreg(0), xreg(1), wreg(2), wreg(3)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/56