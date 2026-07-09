# Bug Report: `encode_ubfm` silently accepts out-of-range `immr`/`imms` immediates

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_ubfm`
**Severity:** High

## Summary

`encode_ubfm` casts immediates to `u32` and ORs directly into word without range validation. `immr = 64` corrupts N bit; `immr = 128` corrupts opcode; negative immediates corrupt entire upper word.

## Root Cause

```rust
let immr = get_imm(operands, 2)? as u32;   // no 0..63 check
let imms = get_imm(operands, 3)? as u32;   // no 0..63 check
let word = ... | (immr << 16) | (imms << 10) | ...;  // overflow corrupts adjacent fields
```

## Reproduction

**Input:** `ubfm x0, x1, #64, #0`

**Expected:** `Err` — UBFM immr out of range (0-63): 64

**Actual:** `Ok(Word(...))` — N bit corrupted (64 << 16 = bit 22)

**Other failing inputs:** `ubfm x0, x1, #128, #0` (corrupts opcode), `ubfm x0, x1, #-1, #0` (wraps to 0xFFFFFFFF)

## Impact

Silent field corruption: out-of-range values overwrite N, opcode, and other adjacent fields. Same defect in `encode_sbfm`, `encode_bfm`, and alias encoders.

## Suggested Fix

Validate before encoding:

```rust
let immr = get_imm(operands, 2)?;
let imms = get_imm(operands, 3)?;
if immr < 0 || immr > 63 {
    return Err(format!("UBFM immr out of range (0-63): {}", immr));
}
if imms < 0 || imms > 63 {
    return Err(format!("UBFM imms out of range (0-63): {}", imms));
}
let immr = immr as u32;
let imms = imms as u32;
```

## Regression Property

Failing property: `prop_rejects_out_of_range_immediates`

```rust
prop_assert!(encode_ubfm(&[xreg(0), xreg(1), imm(64), imm(0)]).is_err());    // immr overflow
prop_assert!(encode_ubfm(&[xreg(0), xreg(1), imm(-1), imm(0)]).is_err());    // negative
prop_assert!(encode_ubfm(&[wreg(0), wreg(1), imm(0), imm(64)]).is_err());    // imms overflow
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/164