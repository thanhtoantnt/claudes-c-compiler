# Bug Report: `encode_neon_movi` `.2d` form emits MVNI opcode bit (bit 29)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_movi`
**Severity:** High

## Summary

`movi v0.2d, #0` emits `0x6F00E400` (bit 29 = 1, MVNI `op` bit) instead of `0x4F00E400`. Combined with `cmode=1110`, `op=1` is UNALLOCATED.

## Root Cause

The `.2d` branch sets top byte to `0x6F` (MVNI prefix) instead of `0x4F` (MOVI).

## Reproduction

**Input:** `movi v0.2d, #0`

**Expected:** `0x4F00E400`

**Actual:** `0x6F00E400` (bit 29 set)

## Impact

Emits UNALLOCATED instruction word — undefined behavior on hardware.

## Suggested Fix

Change `.2d` branch prefix from `0x6F` to `0x4F` (clear bit 29).

## Regression Property

Failing property: `movi_2d_emits_mvni_op_bit`

```rust
prop_assert_eq!(encode_neon_movi_2d(0, 0) & (1 << 29), 0);
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/250
