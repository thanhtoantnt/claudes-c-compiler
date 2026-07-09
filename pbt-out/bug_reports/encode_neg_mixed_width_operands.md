# Bug Report: `encode_neg` silently accepts mixed register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** High

## Summary

`encode_neg` derives `sf` only from `Rd`, discarding width flag for `Rm`. Mixed W/X operands accepted and encoded at destination width instead of being rejected.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rm, _) = get_reg(operands, 1)?;   // width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `neg x0, w1`

**Expected:** `Err` — NEG operands must have matching widths

**Actual:** `Ok(Word(...))` — encodes as 64-bit NEG X0, X1

## Impact

Source typo or macro-generated mixed-width NEG assembles successfully but uses different register width than written.

## Suggested Fix

Compare width flags:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rm, rm_64) = get_reg(operands, 1)?;
if is_64 != rm_64 {
    return Err("NEG operands must have matching widths".to_string());
}
```

## Regression Property

Failing property: `neg_rejects_mixed_width_operands`

```rust
prop_assert!(encode_neg(&[xreg(0), wreg(1)]).is_err());
prop_assert!(encode_neg(&[wreg(0), xreg(1)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/74