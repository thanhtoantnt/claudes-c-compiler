# Bug Report: `encode_neon_ushr` panics on subtraction underflow for large shifts

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ushr`
**Severity:** Medium

## Summary

Per-size subtraction `(16 - shift)` underflows when `shift > 2*esize`, panicking in debug builds before the mask can protect it.

## Root Cause

```rust
"8b" | "16b" => (16 - shift) & 0xF,  // underflow when shift > 16
```

## Reproduction

**Input:** `ushr v0.8b, v1.8b, #20`

**Expected:** `Err`

**Actual:** panic: attempt to subtract with overflow

## Impact

Large malformed shifts abort the assembler instead of returning `Err`.

## Suggested Fix

Validate range before subtraction (same fix as out-of-range sibling).

## Regression Property

Failing property: `prop_overflowing_shifts_must_not_panic`

```rust
prop_assert!(encode_neon_ushr(&[vreg(0,"8b"), vreg(1,"8b"), Imm(20)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/249
