# Bug Report: `encode_clz` silently accepts mismatched register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_clz`
**Severity:** Medium

## Summary

`encode_clz` derives `sf` bit from destination `Rd` only, discarding source `Rn` width. ARMv8-A requires both registers to be same width. Mixed-width forms like `clz x0, w1` accepted without diagnostic.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let word = (is_64 << 31) | (0b010101011 << 21) | (0b00010 << 16) | (rn << 5) | rd;
```

## Reproduction

**Input:** `clz x0, w1`

**Expected:** `Err` — CLZ requires same-width registers

**Actual:** `Ok(Word(...))` — encoded as 64-bit CLZ using W1 number

## Impact

Mixed-width forms accepted silently. Same defect class as `encode_cls`, `encode_clz`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
if is_64 != rn_64 {
    return Err("CLZ requires same-width registers".into());
}
```

## Regression Property

Failing property: `clz_rejects_mixed_width_operands`

```rust
prop_assert!(encode_clz(&[xreg(0), wreg(1)]).is_err());
prop_assert!(encode_clz(&[wreg(0), xreg(1)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/141