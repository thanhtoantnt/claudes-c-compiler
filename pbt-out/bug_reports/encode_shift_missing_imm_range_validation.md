# Bug Report: `encode_shift` immediate form lacks shift-amount range validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_shift`
**Severity:** High

## Summary

`encode_shift` immediate branch never validates `#imm` amount. Out-of-range amount like `lsl w0, w1, #32` panics in debug builds via integer underflow. Same root cause also lets invalid LSR/ASR/ROR amounts and negative amounts be silently mis-encoded.

## Root Cause

```rust
let imm = *imm as u32;  // no range check
let width = if is_64 { 64 } else { 32 };
// LSL branch:
let immr = (width - imm) % width;  // PANIC if imm > width
let imms = width - 1 - imm;        // PANIC if imm >= width
```

## Reproduction

**Input:** `lsl w0, w1, #32`

**Expected:** `Err` — shift amount 32 out of range for 32-bit register

**Actual:** Panic: `attempt to subtract with overflow`

**Minimal failing input:** st = 0, is_64 = false, imm = 32 (boundary case)

## Impact

Debug panic aborts compiler. Release build: silent wraparound, UNDEFINED encoding. Same defect for negative `#imm` (wraps to `0xFFFF_FFFF`).

## Suggested Fix

Validate range before field computation:

```rust
if let Some(Operand::Imm(imm_val)) = operands.get(2) {
    let lo = if shift_type == 0b00 { 0i64 } else { 1 };
    let hi = if is_64 { 63i64 } else { 31i64 };
    if *imm_val < lo || *imm_val > hi {
        return Err(format!("shift amount {} out of range [{}, {}]", imm_val, lo, hi));
    }
}
```

## Regression Property

Failing property: `encode_shift_imm_rejects_out_of_range`

```rust
prop_assert!(encode_shift(&[wreg(0), wreg(1)], 0b00, imm(32)).is_err());  // LSL W #32 out of range
prop_assert!(encode_shift(&[xreg(0), xreg(1)], 0b00, imm(-1)).is_err());  // negative imm
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/95