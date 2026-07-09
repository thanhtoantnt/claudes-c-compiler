# Bug Report: `encode_mul` silently accepts mixed-width source registers

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Severity:** High

## Summary

`encode_mul` derives `sf` (operand-size) bit **only from destination** and performs no check that source registers `Rn`, `Rm` share destination's width. ARMv8 ARM `MUL` alias requires all three operands to be same width; mixed-width form like `mul x0, w1, w2` has no valid encoding and must be rejected.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;   // width from Rd ONLY
let (rn, _) = get_reg(operands, 1)?;        // width discarded
let (rm, _) = get_reg(operands, 2)?;        // width discarded
let sf = sf_bit(is_64);
let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `mul x0, w1, w2`

**Expected:** `Err` — mul operands must all be the same register width

**Actual:** `Ok(Word(0x9B007C00))` — W-register numbers used as if X, no defined semantics

**Minimal failing input:** rd="x0", rn="w0", rm="w0"

## Impact

Assembly source `mul x0, w1, w2` assembled to word rejected by `llvm-mc`. Word uses W-register number as X source, no defined semantics. Same as `encode_smaddl` class bug.

## Suggested Fix

Validate width coherence:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;
if rn_64 != is_64 || rm_64 != is_64 {
    return Err("mul operands must all be the same register width".to_string());
}
```

## Regression Property

Failing property: `mul_rejects_mixed_register_widths`

```rust
prop_assert!(encode_mul(&[xreg(0), wreg(0), wreg(0)]).is_err());
prop_assert!(encode_mul(&[wreg(0), xreg(0), xreg(0)]).is_err());
```

## PBT Results (module `tests::mul`)

| Property | Status |
|---|---|
| `mul_reference_encoding` | PASS |
| `mul_field_placement` | PASS |
| `mul_sf_tracks_destination_width_only` | PASS |
| `mul_rejects_mixed_register_widths` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/70