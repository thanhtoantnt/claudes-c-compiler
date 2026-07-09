# Bug Report: `encode_neon_movi` silently accepts invalid shift kinds

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_movi`
**Severity:** Medium

## Summary

Only LSL and MSL are valid MOVI shift operators. Unrecognized kinds (`lsr`/`asr`/`ror`) fall through to `cmode=0000` and return `Ok` as the unshifted form.

## Root Cause

No validation that `kind` is `"lsl"` or `"msl"`.

## Reproduction

**Input:** `movi v0.4s, #1, lsr #8`

**Expected:** `Err` — lsr not a valid MOVI shift

**Actual:** `Ok` — silently becomes unshifted form

## Impact

Invalid assembly accepted; produces wrong output without diagnostic.

## Suggested Fix

```rust
if kind != "lsl" && kind != "msl" {
    return Err(format!("movi: invalid shift kind '{}'", kind));
}
```

## Regression Property

Failing property: `movi_silently_accepts_invalid_shift_kind`

```rust
prop_assert!(encode_neon_movi_shift("lsr").is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/252
