# Bug Report: `encode_ubfiz` panics / emits UNDEFINED encodings for invalid immediates

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_ubfiz`
**Severity:** High

## Summary

`encode_ubfiz` performs no range validation. `width == 0` causes debug-build panic via underflow. Out-of-range `lsb`/`width` and negative immediates silently produce corrupted encodings. ARM ARM constrains `0 <= lsb <= regsize-1` and `1 <= width <= regsize - lsb`.

## Root Cause

```rust
let lsb = get_imm(operands, 2)? as u32;    // no range check
let width = get_imm(operands, 3)? as u32;  // no range check
let immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
let imms = width - 1;                       // PANIC when width == 0
```

## Reproduction

**Input:** `ubfiz w0, w1, #0, #0`

**Expected:** `Err` — UBFIZ: width 0 invalid (must be >= 1)

**Actual:** Panic: `attempt to subtract with overflow` at bitfield.rs:85

**Minimal failing input:** is_64 = false, lsb = 0, width = 0

## Impact

- Debug-build panic aborts compiler for `width == 0`
- Out-of-range values silently overflow adjacent fields, emitting UNDEFINED encodings
- Same defect in `encode_ubfx`, `encode_ubfm`, `encode_sbfm`, `encode_sbfx`, `encode_sbfiz`, `encode_bfm`, `encode_bfi`, `encode_bfxil`

## Suggested Fix

Validate ranges before computation:

```rust
let regsize = if is_64 { 64u32 } else { 32 };
if lsb >= regsize || width == 0 || lsb + width > regsize {
    return Err(format!("UBFIZ: lsb/width out of range (lsb={}, width={}, regsize={})",
                       lsb, width, regsize));
}
let immr = (regsize - lsb) & (regsize - 1);
let imms = width - 1;  // now safe
```

## Regression Property

Failing property: `prop_rejects_out_of_range_immediates`

```rust
prop_assert!(encode_ubfiz(&[wreg(0), wreg(1), imm(0), imm(0)]).is_err());   // width=0 panic
prop_assert!(encode_ubfiz(&[wreg(0), wreg(1), imm(32), imm(1)]).is_err());  // lsb >= regsize
prop_assert!(encode_ubfiz(&[xreg(0), xreg(1), imm(-1), imm(1)]).is_err()); // negative
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/219