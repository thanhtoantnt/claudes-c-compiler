# Bug Report: `encode_msub` silently accepts mixed-width register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_msub`
**Severity:** High

## Summary

`encode_msub` derives instruction width (`sf`) **only from destination `Rd`**, discarding widths of `Rn`, `Rm`, `Ra` (bound to `_`). AArch64 MSUB requires all four operands to be same width (all 32-bit W or all 64-bit X); mixed W/X operands are undefined and must be rejected.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let (rm, _) = get_reg(operands, 2)?;   // width discarded
let (ra, _) = get_reg(operands, 3)?;   // width discarded
let sf = sf_bit(is_64);              // taken only from Rd
```

## Reproduction

**Input:** `msub x0, w1, x2, x3`

**Expected:** `Err` — msub register operands must have matching widths

**Actual:** `Ok(Word(0x9B028C20))` — sf=1 from x0, hardware executes 64-bit MSUB X0, X1, X2, X3

**Minimal failing input:** rd="x0", rn="w1", rm="x2", ra="x3"

## Impact

Silent mis-assembly: mixed-width `msub` assembled without diagnostic, producing instruction whose execution width contradicts programmer's X notation. GNU `as` rejects with "operand mismatch". Same bug class as `encode_madd`, `encode_div`, `encode_adc`, `encode_sbc`, `encode_eon`.

## Suggested Fix

Capture and validate width of all four operands:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn64)  = get_reg(operands, 1)?;
let (rm, rm64)  = get_reg(operands, 2)?;
let (ra, ra64)  = get_reg(operands, 3)?;
if rn64 != is_64 || rm64 != is_64 || ra64 != is_64 {
    return Err("msub register operands must have matching widths".to_string());
}
```

## Regression Property

Failing property: `msub_rejects_mixed_width_operands`

```rust
prop_assert!(encode_msub(&[xreg(0), wreg(1), xreg(2), xreg(3)]).is_err());
prop_assert!(encode_msub(&[wreg(0), xreg(1), wreg(2), wreg(3)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/68