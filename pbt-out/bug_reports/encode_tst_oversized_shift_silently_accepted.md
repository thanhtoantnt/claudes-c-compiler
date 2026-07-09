# Bug Report: `encode_tst` accepts oversized shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_tst`
**Severity:** Medium

## Summary

`encode_tst` masks shift amount with `& 0x3F` without width-dependent validation. For 32-bit W-registers, shifts 32+ accepted and encoded modulo 64. ARMv8-A restricts TST shifts to 0-31 for W-registers.

## Root Cause

```rust
let shift = (*amount & 0x3F) as u32;  // no width check
```

## Reproduction

**Input:** `tst w0, w1, #64`

**Expected:** `Err` — TST shift out of range: 64 (W-register max is 31)

**Actual:** `Ok(Word(...))` — shift = 64 & 0x3F = 0

**Minimal failing input:** is_64 = false, shift = 64 (or 96, 128, 255)

## Impact

W-register shifts 32+ accepted silently modulo 64. User expects operation at specific shift but gets different encoding.

## Suggested Fix

Validate width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if *amount < 0 || *amount > max_shift {
    return Err(format!("TST shift out of range: {}", *amount));
}
```

## Regression Property

Failing property: `tst_rejects_oversized_shift`

```rust
prop_assert!(encode_tst(&[wreg(0), wreg(1)], 64).is_err());
prop_assert!(encode_tst(&[xreg(0), xreg(1)], 128).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/223