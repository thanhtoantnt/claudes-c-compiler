# Bug Report: `encode_movk` accepts invalid shift amounts for W registers

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_movk`
**Severity:** High

## Summary

For 32-bit `MOVK` instructions, ARMv8-A specifies shift amount of **only `#0` or `#16`**. `encode_movk` masks with `& 0x3` without validation, accepting invalid shifts like `#32`, `#48`, `#64` and encoding them as if they were `#0`.

## Root Cause

```rust
let shift = shift_val & 0x3;  // no range check
```

## Reproduction

**Input:** `movk w0, #0xFFFF, #32`

**Expected:** `Err` — MOVK shift for W-register must be #0 or #16

**Actual:** `Ok(Word(...))` — shift encoded as #0 (32 & 0x3 = 0)

**Minimal failing input:** rd="w0", shift=32 (or 48, 64)

## Impact

Invalid shift values silently coerced to valid shifts. User expects operation at specific bit position but gets different encoding.

## Suggested Fix

Validate shift for W-register forms:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
if !is_64 && (shift_val & 0x3 != 0 && shift_val & 0x3 != 2) {
    return Err("MOVK shift for W-register must be #0 or #16".into());
}
```

## Regression Property

Failing property: `movk_w_register_rejects_invalid_shift`

```rust
prop_assert!(encode_movk(&[wreg(0), imm(0xFFFF), shift(32)]).is_err());  // invalid
prop_assert!(encode_movk(&[wreg(0), imm(0xFFFF), shift(48)]).is_err());  // invalid
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/60