# Bug Report: `encode_neon_rev64` silently encodes unallocated `size=11` arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_rev64`
**Severity:** Medium

## Summary

`encode_neon_rev64` accepts `.1d` and `.2d` arrangements, emitting UNALLOCATED words. REV64 reverses elements within each 64-bit doubleword, so `size = 0b11` is UNDEFINED for this instruction.

## Root Cause

`neon_arr_to_q_size` maps `1d → (Q=0, size=0b11)` and `2d → (Q=1, size=0b11)` with no check that `size == 0b11` is invalid for REV64.

## Reproduction

**Input:** `rev64 v0.1d, v1.1d`

**Expected:** `Err` — REV64 does not support .1d arrangement (size=11 UNDEFINED)

**Actual:** `Ok(Word(0x0EE00820))` — UNALLOCATED encoding

**Minimal failing input:** arr = "1d" (or "2d")

## Impact

UNALLOCATED instruction emitted without diagnostic. llvm-mc-18 rejects these.

## Suggested Fix

Reject `size = 0b11`:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 {
    return Err(format!("REV64 does not support .{} arrangement (size=11 is UNDEFINED)", arr_d));
}
```

## Regression Property

Failing property: `rev64_rejects_unallocated_arrangements`

```rust
prop_assert!(encode_neon_rev64(&[neon_reg(0, "1d"), neon_reg(1, "1d")]).is_err());
prop_assert!(encode_neon_rev64(&[neon_reg(0, "2d"), neon_reg(1, "2d")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/186