# Bug Report: `encode_sbfm` silently accepts out-of-range `immr`/`imms` values

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_sbfm`
**Severity:** High

## Summary

`encode_sbfm` reads immediates as `u32` and ORs directly into 32-bit encoding word without range validation. Per ARMv8-A, `immr` and `immms` are **6-bit fields** `[21:16]` and `[15:10]`. Valid range is `0..=63`. Values outside range corrupts adjacent fields.

## Root Cause

```rust
let immr = get_imm(operands, 2)? as u32;  // no validation
let imms = get_imm(operands, 3)? as u32;
let word = ... | (immr << 16) | (imms << 10) | ...;  // OR directly, no checks
```

## Reproduction

**Input:** `sbfm w0, w0, #64, #64`

**Expected:** `Err` — SBFM immr/imms out of range (valid: 0-63)

**Actual:** `Ok(Word(0x93400000))` — immr=64 corrupts N field (bit 22), imms=64 corrupts Rn field

**Minimal failing input:** immr = 64, imms = 64, neg_imm = -3

## Impact

Out-of-range values silently corrupts adjacent fields. Negative immediates accepted via `as u32` cast, producing enormous positive values.

## Suggested Fix

Validate range before OR:

```rust
let immr = get_imm(operands, 2)?;
let imms = get_imm(operands, 3)?;
if immr < 0 || immr > 63 {
    return Err(format!("SBFM immr must be 0..=63, got {}", immr));
}
if imms < 0 || imms > 63 {
    return Err(format!("SBFM imms must be 0..=63, got {}", imms));
}
let immr = immr as u32;
let imms = imms as u32;
```

## Regression Property

Failing property: `prop_rejects_out_of_range_immediates`

```rust
prop_assert!(encode_sbfm(&[wreg(0), wreg(0)], imm(0), imm(64)]).is_err());  // immr overflow
prop_assert!(encode_sbfm(&[xreg(0), xreg(0)], imm(0), imm(64)]).is_err());  // immr overflow
prop_assert!(encode_sbfm(&[wreg(0), wreg(0)], imm(0), imm(-3)]).is_err());  // negative
```

## PBT Results (module `prop_encode_sbfm_tests`)

| Property | Result |
|---|---|
| `prop_sbfm_field_placement` | PASS |
| `prop_sbfm_equals_sbfx_alias` | PASS |
| `prop_sbfm_xor_ubfm_is_only_bit_30` | PASS |
| `prop_width_changes_only_sf_and_n` | PASS |
| `prop_rejects_malformed_operands` | PASS |
| `prop_rejects_out_of_range_immediates` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/159