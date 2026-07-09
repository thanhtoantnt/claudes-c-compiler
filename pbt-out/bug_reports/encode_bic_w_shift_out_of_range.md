# Bug Report: `encode_bic` W-register shifted form silently accepts shift ≥ 32

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_bic`
**Severity:** Medium

## Summary

In the scalar shifted-register branch, the shift amount is masked into the 6-bit `imm6` field with `& 0x3F` for *both* 32-bit (W) and 64-bit (X) forms, with no width-based range check. For a 64-bit register, `imm6 ∈ [0, 63]` is valid. For a 32-bit register (`sf == 0`), the ARMv8 ARM restricts `imm6` to `[0, 31]`; any larger value is UNDEFINED. This implementation silently encodes invalid values.

## Root Cause

```rust
let word = (sf << 31) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
         | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

The `& 0x3F` mask is applied without checking the value against the valid range for the register width.

## Reproduction

**Input:** `bic w0, w0, w0, lsl #32`

**Expected:** `Err` — shift amount out of range (0..=31 for W-registers)

**Actual:** `Ok(EncodeResult::Word(..))` — emits UNDEFINED encoding

**Minimal failing input:** rd = 0, rn = 0, rm = 0, amount = 32, sk = 0

## Impact

Emits architecturally UNDEFINED encodings. For 32-bit registers, shift amounts 32..=63 are UNDEFINED (may behave unpredictably on hardware). Same defect class as `encode_add_sub` and `encode_mvn`.

## Suggested Fix

Validate the shift amount against register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-bit register (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```

## Regression Property

Failing property: `bic_w_register_rejects_shift_above_31`

```rust
prop_assert!(encode_bic(&[wreg(rd), wreg(rn), wreg(rm)], "lsl", 32).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/10