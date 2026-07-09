# Bug Report: `encode_neon_movi` `.2d` rejects standard replicate immediate form

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_movi`
**Severity:** Medium

## Summary

ARMv8-A `MOVI Vd.2D, #imm` (cmode=1110, op=0) replicates `imm8` to every byte. The `.2d` branch requires every byte to be `0x00`/`0xFF` and rejects valid replicate forms like `#0x0101010101010101`.

## Root Cause

Non-standard byte-mask validation instead of ARM ARM replicate semantic.

## Reproduction

**Input:** `movi v0.2d, #0x0101010101010101`

**Expected:** `Ok` — standard replicate form (imm8=0x01)

**Actual:** `Err` — rejected as invalid

## Impact

Valid MOVI .2d instructions rejected; code generation cannot use the form.

## Suggested Fix

Accept imm8 replicate form per ARM ARM (any byte 0x00–0xFF, replicated).

## Regression Property

Failing property: `movi_2d_rejects_replicate_form`

```rust
prop_assert!(encode_neon_movi_2d(0, 0x0101010101010101).is_ok());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/251
