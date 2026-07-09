# Bug Report: `encode_add_sub` W-register shifted form silently accepts shift ≥ 32

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_add_sub`
**Severity:** Medium

## Summary

In the shifted-register branch, the shift amount is masked into the 6-bit `imm6` field with `& 0x3F` for *both* 32-bit (W) and 64-bit (X) forms:

```rust
let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24) | (shift_type << 22))
         | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

For a 64-bit register, `imm6 ∈ [0, 63]` is valid, so masking is correct. For a 32-bit register (`sf == 0`), the ARMv8 ARM restricts `imm6` to `[0, 31]`; any larger value is UNDEFINED. GAS and LLVM-MC reject such encodings (e.g. `add w0, w0, w0, lsl #32`). This implementation instead silently encodes `imm6 = 32`, producing a word that is architecturally UNDEFINED.

## Root Cause

The `& 0x3F` mask is applied to the shift amount without first checking if the value exceeds the valid range for the register width. For 32-bit registers, shift amounts 32..=63 should be rejected but are silently masked.

## Reproduction

**Input:** `add w0, w0, w0, lsl #32`

**Expected:** `Err` — shift amount out of range (0..=31 for W-registers)

**Actual:** `Ok(EncodeResult::Word(..))` — emits UNDEFINED encoding

**Minimal failing input:** rd = 0, rn = 0, rm = 0, amount = 32, sk = 0

## Impact

Emits architecturally UNDEFINED encodings. For 32-bit registers, shift amounts 32..=63 are UNDEFINED (may behave unpredictably on hardware). The assembler accepts invalid instructions and produces words that hardware treats as undefined behavior.

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

Failing property: `w_reg_shifted_form_rejects_shift_above_31`

```rust
prop_assert!(encode_add_sub(&[wreg(rd), wreg(rn), wreg(rm)], "lsl", 32], false, false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/7