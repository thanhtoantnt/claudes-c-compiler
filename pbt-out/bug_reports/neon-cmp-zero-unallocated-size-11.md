# Bug Report: `encode_neon_cmp_zero` accepts unallocated `size=11` arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_cmp_zero`
**Severity:** Medium

## Summary

`encode_neon_cmp_zero` doesn't reject arrangements mapping to `size = 0b11` (`.1d`, `.2d`). Compare-to-zero encoding requires `size ∈ {00, 01, 10}` only. `size = 11` is UNALLOCATED.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;  // 1d/2d -> size=11
let word = ... | (size << 22) | ...;           // no check against size==11
```

## Reproduction

**Input:** `cmeq v0.2d, v1.2d, #0`

**Expected:** `Err` — CMEQ (compare to zero) does not support .2d arrangement

**Actual:** `Ok(Word(...))` — unallocated encoding emitted

**Minimal failing input:** arr_d = "2d" (or "1d")

## Impact

UNALLOCATED instructions emitted without diagnostic. Hardware behavior undefined.

## Suggested Fix

Reject unallocated arrangements:

```rust
if size == 0b11 {
    return Err(format!("cmeq (compare to zero) does not support .{} arrangement", arr_d));
}
```

## Regression Property

Failing property: `rejects_unallocated_size_11_arrangements`

```rust
prop_assert!(encode_neon_cmp_zero("cmeq", &[neon_reg(0, "2d"), neon_reg(1, "2d")]).is_err());
prop_assert!(encode_neon_cmp_zero("cmge", &[neon_reg(0, "1d"), neon_reg(1, "1d")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/184