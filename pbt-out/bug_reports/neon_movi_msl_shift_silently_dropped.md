# Bug Report: `encode_neon_movi` silently drops MSL shift on `.2s`/`.4s`

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_movi`
**Severity:** Medium

## Summary

MSL is valid for 32-bit MOVI (`msl #8` → cmode=1100, `msl #16` → cmode=1101). The `.2s`/`.4s` branch only matches `kind == "lsl"`, so MSL falls through to `cmode=0000` and the shift is dropped.

## Root Cause

`.2s`/`.4s` branch checks `kind == "lsl"` but does not handle `kind == "msl"`.

## Reproduction

**Input:** `movi v0.4s, #1, msl #8`

**Expected:** cmode=1100

**Actual:** cmode=0000 (unshifted)

## Impact

Silent misencoding — instruction executes as unshifted form.

## Suggested Fix

Handle `msl #8` / `msl #16` in the 32-bit branch.

## Regression Property

Failing property: `movi_drops_msl_shift_on_32bit`

```rust
prop_assert_eq!(cmode_of(encode_neon_movi_msl(8)), 0b1100);
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/253
