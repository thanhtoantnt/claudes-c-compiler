# Bug Report: `encode_sbfx` panics on `SBFX Rd, Rn, #0, #0` (width zero underflow)

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_sbfx`
**Severity:** High

## Summary

`SBFX Rd, Rn, #0, #0` causes compiler panic due to arithmetic underflow in `lsb + width - 1`. ARM ARM requires `1 <= width <= regsize - lsb`, so `width == 0` must be rejected.

## Root Cause

```rust
let imms = lsb + width - 1;   // underflow when width = 0 and lsb = 0
```

With `lsb = 0` and `width = 0`: `0u32 + 0u32 - 1` = underflow → panic.

## Reproduction

**Input:** `sbfx w0, w1, #0, #0`

**Expected:** `Err` — SBFX: width 0 invalid (must be >= 1)

**Actual:** Panic: `attempt to subtract with overflow`

**Minimal failing input:** lsb = 0, width = 0 (any register width)

## Impact

Any source containing `SBFX Rd, Rn, #0, #0` or `SBFX Rd, Rn, #n, #0` crashes compiler instead of producing diagnostic.

## Suggested Fix

Validate width >= 1 before computation:

```rust
if width == 0 {
    return Err(format!("SBFX: width 0 invalid (must be >= 1)"));
}
let imms = lsb + width - 1;  // now safe: width >= 1
```

## Regression Property

Failing property: `prop_rejects_zero_width`

```rust
prop_assert!(encode_sbfx(&[xreg(0), xreg(1), imm(0), imm(0)]).is_err());  // width=0 panic
prop_assert!(encode_sbfx(&[wreg(0), wreg(1), imm(0), imm(0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/218