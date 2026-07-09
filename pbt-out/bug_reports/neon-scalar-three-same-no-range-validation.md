# Bug Report: `encode_neon_scalar_three_same` silently accepts out-of-range parameters

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_scalar_three_same`
**Severity:** High

## Summary

`encode_neon_scalar_three_same` OR-shifts `u_bit`, `size`, and `opcode` into instruction word without range validation. Out-of-range values silently corrupt adjacent fixed bits.

## Root Cause

```rust
let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22) | (1 << 21)
    | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
```

- `u_bit` is 1-bit (0-1): value 2+ corrupts bit 28 (start of `11110` opcode)
- `size` is 2-bit (0-3): value 4+ corrupts `u_bit`
- `opcode` is 5-bit (0-31): value 32+ corrupts `const` bit 10

## Reproduction

**Input:** `encode_neon_scalar_three_same(&[dreg(0), dreg(1), dreg(2)], 2, 0b11010, 0)`

**Expected:** `Err` — u_bit out of range (must be 0 or 1)

**Actual:** `Ok(Word(...))` — bit 28 corrupted, wrong instruction

## Impact

UNALLOCATED/corrupted encodings emitted without diagnostic. All three fields lack bounds checks.

## Suggested Fix

Validate all parameters before encoding:

```rust
if u_bit > 1 { return Err("u_bit out of range (0-1)".into()); }
if size > 3 { return Err("size out of range (0-3)".into()); }
if opcode > 31 { return Err("opcode out of range (0-31)".into()); }
```

## Regression Property

Failing property: `neon_scalar_three_same_rejects_out_of_range_params`

```rust
prop_assert!(encode_neon_scalar_three_same(&[dreg(0), dreg(1), dreg(2)], 2, 0, 0).is_err());  // u_bit
prop_assert!(encode_neon_scalar_three_same(&[dreg(0), dreg(1), dreg(2)], 0, 0, 4).is_err());  // size
prop_assert!(encode_neon_scalar_three_same(&[dreg(0), dreg(1), dreg(2)], 0, 32, 0).is_err()); // opcode
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/189