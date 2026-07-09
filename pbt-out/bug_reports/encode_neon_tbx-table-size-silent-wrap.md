# Bug Report: `encode_neon_tbx` silently wraps out-of-range table size

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** High

## Summary

TBX allows 1–4 table registers. The encoder masks `(num_regs - 1) & 0x3`, so a 5-register table encodes identically to a 1-register table.

## Root Cause

```rust
let len = (num_regs - 1) & 0x3;  // wraps 5..8 -> 0..3
```

## Reproduction

**Input:** `tbx v0.16b, {v1.16b-v5.16b}, v6.16b`

**Expected:** `Err` — llvm-mc: invalid number of vectors

**Actual:** `Ok(Word(...))` identical to 1-register form

## Impact

Larger tables silently assemble as smaller lookups — invisible correctness bug.

## Suggested Fix

```rust
if !(1..=4).contains(&num_regs) {
    return Err(format!("tbx: table must have 1-4 registers, got {}", num_regs));
}
```

## Regression Property

Failing property: `tbx_rejects_table_larger_than_four_regs`

```rust
prop_assert!(encode_neon_tbx_with_n_regs(5).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/247
