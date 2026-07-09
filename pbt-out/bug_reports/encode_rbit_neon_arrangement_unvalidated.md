# Bug Report: `encode_rbit` silently accepts invalid NEON vector arrangements

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rbit`
**Severity:** High

## Summary

NEON `RBIT` is constrained to `<T> ∈ {8B, 16B}` only. `encode_rbit` silently accepts any arrangement by defaulting to `.8b`, producing UNALLOCATED encodings for `.4h`, `.8h`, `.2s`, `.4s`.

## Root Cause

```rust
let q: u32 = if arr_d == "16b" { 1 } else { 0 };  // no validation; defaults to 8B
let word = (q << 30) | (1 << 29) | (0b01110 << 24) | ...;
```

Source arrangement discarded entirely.

## Reproduction

**Input:** `rbit v0.4h, v1.4h`

**Expected:** `Err` — RBIT vector form supports only .8b and .16b

**Actual:** `Ok(Word(...))` — silently encoded as `.8b` form

**Minimal failing input:** arr_d = "4h" (or "8h", "2s", "4s", "1d", "2d")

## Impact

Non-byte arrangements silently accepted, producing UNALLOCATED encodings. Same pattern in `encode_rev` NEON path.

## Suggested Fix

Reject invalid arrangements:

```rust
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("RBIT vector form supports only .8b and .16b, got {}", arr_d));
}
```

## Regression Property

Failing property: `prop_rejects_invalid_vector_arrangements`

```rust
prop_assert!(encode_rbit(&[neon_reg(0, "4h"), neon_reg(1, "4h")]).is_err());
prop_assert!(encode_rbit(&[neon_reg(0, "2s"), neon_reg(1, "2s")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/152