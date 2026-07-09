# Bug Report: `encode_neon_scalar_qshrn` emits UNDEFINED encoding (bit 28 cleared)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_scalar_qshrn`
**Severity:** High

## Summary

The scalar saturating-shift-right-narrow encoders (SQSHRN/SQRSHRN/UQSHRN/UQRSHRN) emit `0b011110` for bits [28:23] but the correct AArch64 scalar shift encoding requires `0b111110` (bit 28 must be 1). This produces UNDEFINED/reserved encodings on real hardware and disagrees with llvm-mc/gas/as.

## Root Cause

```rust
// current (wrong):
let word = (0b01 << 30) | (u_bit << 29) | (0b011110 << 23) | ...
//                              bit28=0 ^^^^

// should be:
let word = (0b01 << 30) | (u_bit << 29) | (0b111110 << 23) | ...
//                              bit28=1 ^^^^
```

The layout is `01 U 11110 ...` but the code uses `011110`, omitting the mandatory `1` at bit 28.

## Reproduction

**Input:** `sqshrn b0, h0, #1`

**Expected:** `0x5F0F9400` (llvm-mc-18)

**Actual:** `0x4F0F9400` (XOR = 0x10000000, bit 28 wrong)

**Minimal failing input:** any scalar QSHRN-family instruction

## Impact

Every emitted scalar QSHRN-family instruction is malformed and UNDEFINED on hardware. The encoder is internally consistent, so the bad word is silently written into object files.

## Suggested Fix

Change `0b011110 << 23` to `0b111110 << 23`:

```rust
let word = (0b01 << 30) | (u_bit << 29) | (0b111110 << 23)
    | (immh << 19) | (immb << 16) | (rn << 5) | rd;
```

## Regression Property

Failing property: `prop_matches_llvm_mc`

```rust
prop_assert_eq!(encode_neon_scalar_qshrn(&[breg(0), hreg(0)], 0, 1, 0b000),
               0x5F0F9400);  // bit 28 = 1
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/224