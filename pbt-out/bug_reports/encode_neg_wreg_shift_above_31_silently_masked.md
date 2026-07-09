# Bug Report: `encode_neg` silently masks W-register shifts above 31

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** High

## Summary

`encode_neg` masks shift amount with `& 0x3F` without width-dependent validation. For 32-bit W registers, shift amounts 32+ accepted and encoded as `amount % 64`, producing invalid instruction.

## Root Cause

```rust
let imm6 = (*amount & 0x3F) as u32;  // no width check
```

## Reproduction

**Input:** `neg w0, w1, lsl #32`

**Expected:** `Err` — neg shift out of range: 32 (W-register max is 31)

**Actual:** `Ok(Word(...))` — imm6 = 32 & 0x3F = 32

**Minimal failing input:** is_64 = false, amount = 32 (or 64, 96, etc.)

## Impact

W-register shifts 32+ accepted, encoded as invalid instructions. ARM requires shifts 0-31 for W-registers.

## Suggested Fix

Validate width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if *amount < 0 || *amount > max_shift {
    return Err(format!("neg shift out of range: {}", amount));
}
```

## Regression Property

Failing property: `neg_wreg_rejects_shift_above_31`

```rust
prop_assert!(encode_neg(&[wreg(0), wreg(1), shift("lsl", 32)]).is_err());
prop_assert!(encode_neg(&[wreg(0), wreg(1), shift("lsl", 63)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/78