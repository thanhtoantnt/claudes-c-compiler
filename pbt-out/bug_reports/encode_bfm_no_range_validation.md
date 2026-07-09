# Bug Report: `encode_bfm` accepts out-of-range `immr`/`imms` and silently corrupts the opcode

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_bfm`
**Severity:** High

## Summary

`encode_bfm` casts parsed immediates with `as u32` and ORs directly into instruction word without range validation. Overflow bits spill into adjacent fixed/structural fields: `immr = 64` sets bit 22 (`N`), violating `N == sf`; `imms = 64` sets bit 16 (low bit of `immr`).

## Root Cause

```rust
let immr = get_imm(operands, 2)? as u32;   // no range check
let imms = get_imm(operands, 3)? as u32;   // no range check
let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22)
         | (immr << 16) | (imms << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `bfm x0, x0, #64, #0`

**Expected:** `Err` — immr 64 out of range [0,63]

**Actual:** `Ok(Word(0xB3400000))` — N silently flipped to 1 (violates N==sf for 32-bit)

**Minimal failing input:** immr = 64, imms = 0, is_64 = true

## Impact

Silent opcode corruption. For 32-bit registers (`sf=0`, N should be 0), `immr=64` flips N to 1 → unallocated encoding. `as u32` on negative immediates wraps to `0xFFFF_FFFF`, ORing 1s across all fields. Same defect in `encode_ubfm`, `encode_sbfm`, and alias encoders.

## Suggested Fix

Validate immediates before encoding:

```rust
if immr < 0 || immr > 63 {
    return Err(format!("BFM: immr {} out of range [0,63]", immr));
}
if imms < 0 || imms > 63 {
    return Err(format!("BFM: imms {} out of range [0,63]", imms));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_immediates`

```rust
prop_assert!(encode_bfm(&[xreg(0), xreg(0), imm(64), imm(0)]).is_err());
prop_assert!(encode_bfm(&[xreg(0), xreg(0), imm(-1), imm(0)]).is_err());
```

## PBT Results (module `prop_encode_bfm_tests`)

| Property | Result |
|---|---|
| `prop_bfm_field_placement` | PASS |
| `prop_bfm_xor_siblings` | PASS |
| `prop_bfm_equals_bfxil_alias` | PASS |
| `prop_width_changes_only_sf_and_n` | PASS |
| `prop_rejects_out_of_range_immediates` | **FAIL** |
| `prop_rejects_malformed_operands` | PASS |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/158