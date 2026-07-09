# Bug Report: `encode_neon_shift_imm` ignores `_is_unsigned` — SSHR unreachable

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_shift_imm`
**Severity:** Medium

## Summary

`encode_neon_shift_imm` has `_is_unsigned: bool` parameter that is silently ignored. U bit (bit 29) hardcoded to 1, making function always emit unsigned shifts (USHR/URSHR/URSRA). Signed variants (SSHR/SRSHR/SRSRA, U=0) are unreachable.

## Root Cause

```rust
let u: u32 = 1;  // hardcoded — _is_unsigned parameter ignored
let word = (q << 30) | (u << 29) | ...;
```

## Reproduction

**Input:** `encode_neon_shift_imm(&ops, false)` (requesting signed shift)

**Expected:** SSHR encoding with U=0 bit

**Actual:** Same word as `encode_neon_shift_imm(&ops, true)` — U bit always 1

**Minimal failing input:** rd=0, rn=1, shift=1, arr="8h"

## Impact

SSHR/SRSHR/SRSRA instructions can never be generated via this path. Callers passing `false` silently get the unsigned variant instead. Effectively a dead API parameter.

## Suggested Fix

Wire the parameter to the U bit:

```rust
let u: u32 = if is_unsigned { 1 } else { 0 };
let word = (q << 30) | (u << 29) | ...;
```

## Regression Property

Failing property: `prop_is_unsigned_ignored`

```rust
prop_assert_ne!(encode_neon_shift_imm(&ops, true), encode_neon_shift_imm(&ops, false));
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/83