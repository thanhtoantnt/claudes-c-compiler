# Bug Report: `encode_sbfx` performs no range validation on `lsb` / `width`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_sbfx`
**Severity:** High

## Summary

`encode_sbfx` accepts any `i64` for `lsb` and `width` with no range validation. Values that are negative, exceed register size, or make `lsb + width` exceed register size silently produce corrupted encodings.

## Root Cause

```rust
let lsb = get_imm(operands, 2)? as u32;     // no range check
let width = get_imm(operands, 3)? as u32;   // no range check
let immr = lsb;
let imms = lsb + width - 1;
let word = ... | (immr << 16) | (imms << 10) | ...;
```

## Reproduction

**Input:** `sbfx w0, w1, #32, #1`

**Expected:** `Err` — SBFX: lsb 32 out of range [0, 32)

**Actual:** `Ok(Word(...))` — immr = 32, corrupts N field (bit 22)

**Other failing inputs:** `sbfx x0, x1, #0, #65` (width too large), `sbfx x0, x1, #-3, #1` (negative wraps)

## Impact

Silent mis-encoding: out-of-range values corrupt adjacent fields. `immr >= 64` overflows into N bit; `imms >= 64` overflows into Rn field; negatives wrap to huge values, corrupting opcode bits.

## Suggested Fix

Validate ranges before encoding:

```rust
let regsize = if is_64 { 64 } else { 32 };
if lsb >= regsize {
    return Err(format!("SBFX: lsb {} out of range [0, {})", lsb, regsize));
}
if width == 0 || lsb + width > regsize {
    return Err(format!("SBFX: width {} invalid for lsb {} (regsize {})", width, lsb, regsize));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_lsb_width`

```rust
prop_assert!(encode_sbfx(&[wreg(0), wreg(1), imm(32), imm(1)]).is_err());  // lsb >= regsize
prop_assert!(encode_sbfx(&[xreg(0), xreg(1), imm(0), imm(65)]).is_err());  // width > regsize
prop_assert!(encode_sbfx(&[xreg(0), xreg(1), imm(-3), imm(1)]).is_err());  // negative lsb
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/217