# Bug Report: `encode_neon_sshr` panics on oversized / negative shift immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sshr`
**Severity:** Medium

## Summary

`encode_neon_sshr` computes `immh:immb` via `(2*esize - shift)` without validating `shift`. Negative or oversized shifts cause unsigned underflow and panic under `debug_assertions`.

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;   // -1 -> u32::MAX
"8b" | "16b" => (16 - shift) & 0xF,         // underflow -> panic
```

## Reproduction

**Input:** `sshr v0.8b, v1.8b, #17`

**Expected:** `Err` — shift out of range

**Actual:** panic: attempt to subtract with overflow

## Impact

Malformed immediates abort the assembler in debug builds instead of returning a diagnostic.

## Suggested Fix

```rust
if shift_i < 1 || shift as u32 > esize {
    return Err(format!("sshr: shift {} out of range [1, {}]", shift_i, esize));
}
```

## Regression Property

Failing property: `sshr_returns_err_on_overflow_shift`

```rust
prop_assert!(encode_neon_sshr(&[vreg(0, "8b"), vreg(1, "8b"), Imm(17)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/243
