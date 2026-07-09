# Bug Report: `encode_neon_float_cmp_zero` accepts unallocated `size_hi` values

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_float_cmp_zero`
**Severity:** High

## Summary

`encode_neon_float_cmp_zero` (FCMEQ/FCMGE/FCMGT/FCMLE/FCMLT vector #0.0) accepts `size_hi = 1`, which produces `size = 0b10` (or `0b11`) — UNALLOCATED for this instruction group. Per ARMv8-A, the `size` field [23:22] must be `00` (single) or `01` (double); bit[23] must be 0.

## Root Cause

```rust
let size = (size_hi << 1) | sz;  // size_hi validated nowhere
let word = ... | (size << 22) | ...;
```

Non-zero `size_hi` produces unallocated `size = 0b10` or `0b11`.

## Reproduction

**Input:** `encode_neon_float_cmp_zero(&[vreg(0, "2s"), vreg(0, "2s")], u_bit=0, size_hi=1, opcode=0)`

**Expected:** `Err` — size_hi must be 0 (bit[23] must be 0)

**Actual:** `Ok(Word(0x0EA00820))` — bits[23:22] = 10, UNALLOCATED

**Minimal failing input:** size_hi=1 (any non-zero)

## Impact

Silent mis-assembly. Reachable in practice: `mod.rs` passes `size_hi = 1` for FCMGT/FCMLT, so `fcmgt v0.4s, v1.4s, #0.0` silently yields UNALLOCATED encoding.

## Suggested Fix

Validate `size_hi`:

```rust
if size_hi != 0 {
    return Err(format!("float cmp zero: size_hi must be 0 (got {})", size_hi));
}
```

## Regression Property

Failing property: `prop_rejects_unallocated_size_hi`

```rust
prop_assert!(encode_neon_float_cmp_zero(&[vreg(0, "2s"), vreg(0, "2s")], 0, 1, 0).is_err());  // size_hi=1
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/45