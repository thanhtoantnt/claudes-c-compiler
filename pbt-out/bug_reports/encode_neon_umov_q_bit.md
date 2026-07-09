# Bug Report: `encode_neon_umov` accepts dead `q` bit

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_umov`
**Severity:** Low

## Summary

`encode_neon_umov` has a dead `q` parameter derived from destination arrangement but never used in encoding. The `q` bit is fixed at bit 30 for UMOV instructions, so the derived value is discarded.

## Root Cause

```rust
let (q, _) = neon_arr_to_q_size(&arr_d)?;  // q computed but unused
let word = (1u32 << 31) | (q << 30) | ...     // q masked out by fixed 1
```

The `q << 30` is overridden by `1u32 << 31`, making the derived `q` dead.

## Impact

Dead parameter adds confusion but no functional bug. Could be removed as cleanup.

## Suggested Fix

Remove dead parameter and directly set fixed bits:

```rust
let word = (1u32 << 31) | (size << 22) | (imm3 << 19) | (1 << 18)
       | (imm4 << 16) | (imm5 << 11) | (0b101 << 6) | (lane << 5) | rd;
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/90