# Bug Report: `encode_eon` silently accepts W-register shifts above 31

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_eon`
**Severity:** High

## Summary

`encode_eon` masks shift amount with `& 0x3F` without width-dependent validation. For 32-bit W-register forms, ARM permits shifts only in `0..=31`; amounts `32..=63` accepted and encoded as UNPREDICTABLE.

## Root Cause

```rust
let imm6 = shift_amount & 0x3F;  // no width check
```

## Reproduction

**Input:** `eon w0, w0, w0, lsl #32`

**Expected:** `Err` — eon shift out of range: 32 (W-register max is 31)

**Actual:** `Ok(Word(...))` — encoded as UNPREDICTABLE instruction

**Minimal failing input:** shift_amount = 32, is_64 = false

## Impact

Invalid shifted-register sources accepted, undefined 32-bit encodings emitted.

## Suggested Fix

Validate width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount < 0 || shift_amount > max_shift {
    return Err(format!("eon shift out of range: {}", shift_amount));
}
```

## Regression Property

Failing property: `eon_w_register_rejects_shift_above_31`

```rust
prop_assert!(encode_eon(&[wreg(0), wreg(0), wreg(0), shift("lsl", 32)]).is_err());
prop_assert!(encode_eon(&[wreg(0), wreg(0), wreg(0), shift("lsl", 63)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/40