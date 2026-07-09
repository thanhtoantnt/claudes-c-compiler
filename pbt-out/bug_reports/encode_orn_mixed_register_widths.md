# Bug Report: `encode_orn` silently accepts mixed-width register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`
**Severity:** High

## Summary

`encode_orn` derives `sf` (width) bit **only from destination `Rd`**, discarding widths of `Rn` and `Rm`. ARMv8-A ORN (shifted register) requires all three operands to share same width. Mixed-width forms like `orn x0, w1, w2` accepted and encoded with `sf=1`.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let (rm, _) = get_reg(operands, 2)?;   // width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `orn x0, w1, w2, lsl #0`

**Expected:** `Err` — all ORN operands must have matching widths

**Actual:** `Ok(Word(...))` — sf=1 from Rd, W-register numbers used

**Minimal failing input:** rd="x0", rn="w0", rm="w0", shift=0

## Impact

Mixed-width forms accepted. Same defect class as `encode_eon`, `encode_eor`, `encode_and`.

## Suggested Fix

Collect and validate width flags:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;
if rn_64 != is_64 || rm_64 != is_64 {
    return Err("all ORN operands must have the same width".into());
}
```

## Regression Property

Failing property: `orn_rejects_mixed_width_operands`

```rust
prop_assert!(encode_orn(&[xreg(0), wreg(1), wreg(2)], shift("lsl", 0)]).is_err());
prop_assert!(encode_orn(&[wreg(0), xreg(1), xreg(2)], shift("lsl", 0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/91