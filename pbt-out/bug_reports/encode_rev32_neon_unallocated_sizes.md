# Bug Report: `encode_rev32` (NEON) accepts UNALLOCATED doubleword arrangements

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rev32`
**Severity:** Medium

## Summary

NEON `REV32 <Vd>.<T>` is only valid for `size ∈ {00 (bytes), 01 (halfwords)}`. The NEON branch accepts any arrangement `neon_arr_to_q_size` recognizes, including `.2s`/`.4s` (size=10) and `.1d`/`.2d` (size=11), producing UNALLOCATED encodings.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;  // no validation of size field
let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | ...;
```

## Reproduction

**Input:** `rev32 v0.4s, v1.4s`

**Expected:** `Err` — REV32 vector form supports only .8b, .16b, .4h, .8h arrangements

**Actual:** `Ok(Word(...))` — accepted with size=10 (UNALLOCATED)

**Minimal failing input:** arr_d = "4s" (or "2s", "1d", "2d")

## Impact

UNALLOCATED encodings emitted without diagnostic. Reference assemblers reject these.

## Suggested Fix

Validate arrangement:

```rust
let valid = ["8b", "16b", "4h", "8h"];
if !valid.contains(&arr_d.as_str()) {
    return Err(format!("REV32 vector form supports only .8b/.16b/.4h/.8h, got {}", arr_d));
}
```

## Regression Property

Failing property: `prop_neon_rejects_unallocated_sizes`

```rust
prop_assert!(encode_rev32(&[neon_reg(0, "4s"), neon_reg(1, "4s")]).is_err());  // size=10 UNALLOCATED
prop_assert!(encode_rev32(&[neon_reg(0, "2d"), neon_reg(1, "2d")]).is_err());  // size=11 UNALLOCATED
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/154