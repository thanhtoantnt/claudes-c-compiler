# Bug Report: `encode_neon_zip_uzp` silently encodes unallocated `.1d` arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_zip_uzp`
**Severity:** Medium

## Summary

`encode_neon_zip_uzp` silently encodes `.1d` arrangement (size=11, Q=0) as UNALLOCATED. ARMv8-A defines `UZP1/UZP2/ZIP1/ZIP2/TRN1/TRN2` for `size=11, Q=0` as UNDEFINED. Only `.2d` (size=11, Q=1) is valid.

## Root Cause

`neon_arr_to_q_size("1d") = (Q=0, size=11)` without validation that `size=11, Q=0` is UNALLOCATED for this instruction class.

## Reproduction

**Input:** `uzp1 v0.1d, v1.1d, v2.1d`

**Expected:** `Err` — UZP1 does not support .1d arrangement (size=11, Q=0 is UNALLOCATED)

**Actual:** `Ok(Word(0x0EC01800))` — UNALLOCATED encoding accepted

## Impact

UNALLOCATED instruction emitted without diagnostic. llvm-mc-18 rejects with "invalid operand for instruction".

## Suggested Fix

Reject `.1d` arrangement:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 && q == 0 {
    return Err(format!("{} does not support .1d arrangement (UNALLOCATED)", mnemonic));
}
```

## Regression Property

Failing property: `zip_uzp_rejects_unallocated_1d_arrangements`

```rust
prop_assert!(encode_neon_zip_uzp(&[neon_reg(0, "1d"), neon_reg(1, "1d"), neon_reg(2, "1d")], 0b001, false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/187