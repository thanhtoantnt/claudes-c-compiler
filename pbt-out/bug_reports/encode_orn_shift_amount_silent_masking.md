# Bug Report: `encode_orn` silently masks out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`
**Severity:** Medium

## Summary

`encode_orn` unconditionally masks the shift amount with `& 0x3F` without range-checking it against the register width.

## Spec (ARMv8 ARM §C4.1.115)

The shifted-register ORN `imm6` field (bits 15:10) holds the shift amount:
- 64-bit (X) registers: valid range `0..=63`; `lsl #64` and above are UNDEFINED
- 32-bit (W) registers: valid range `0..=31`; `32..=63` is UNPREDICTABLE

GAS and llvm-mc reject these with `Error: immediate value out of range`.

## Root Cause

```rust
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

No validation that the shift amount fits the architectural range before masking.

## Reproduction

**Input:** `orn x0, x0, x0, lsl #64`

**Expected:** `Err` — shift amount out of range (0..=63 for X-registers)

**Actual:** `Ok(Word(_))` — encodes identically to `orn x0, x0, x0` (lsl #64 → lsl #0)

**Minimal failing input:** rd = 0, rn = 0, rm = 0, amount = 64

## Impact

Silent mis-compilation: any caller passing an out-of-range shift emits a different instruction than intended with no diagnostic.

## Suggested Fix

Range-check the shift against the width:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for orn", shift_amount));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/87