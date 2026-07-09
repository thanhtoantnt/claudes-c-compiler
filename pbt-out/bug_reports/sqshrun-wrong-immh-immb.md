# Bug Report: `encode_neon_sqshrun` wrong `immh:immb` field

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sqshrun`
**Severity:** High

## Summary

`immh:immb` is computed as `element_bits - shift` instead of `2*esize - shift`. The emitted instruction decodes as a different (or UNALLOCATED) operation.

## Root Cause

```rust
let immhb = (element_bits - shift) & 0x7F; // should be 2*element_bits - shift
let immh = (immhb >> 3) | immh_base;       // immh_base also wrong
```

## Reproduction

**Input:** `sqshrun v0.8b, v1.8h, #1`

**Expected:** `0x2F1F8420` (immh:immb = 0x1F)

**Actual:** `0x2F0F8420` (immh:immb = 0x0F)

## Impact

Silent code corruption — word decodes as different instruction or UNALLOCATED.

## Suggested Fix

```rust
let immhb = element_bits * 2 - shift;
let immh = (immhb >> 3) & 0xF;
let immb = immhb & 0x7;
```

## Regression Property

Failing property: `immh_immb_matches_arm_spec`

```rust
prop_assert_eq!(encode_neon_sqshrun(...), Ok(Word(0x2F1F8420)));
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/255
