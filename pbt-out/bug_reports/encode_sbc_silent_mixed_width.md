# Bug Report: `encode_sbc` silently accepts mismatched operand widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sbc`
**Severity:** Medium

## Summary

`encode_sbc` derives the instruction width (`sf`, bit 31) only from the destination operand `Rd`. The source operand widths from `Rn` and `Rm` are discarded by `let (rn, _) = get_reg(...)` / `let (rm, _) = ...`. Mixed W/X operands are therefore silently encoded using the destination width.

## Root Cause

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
```

## Reproduction

**Input:** `sbc x0, w1, x2`

**Expected:** `Err` — operand size mismatch

**Actual:** `Ok(Word(_))` — encodes as `sbc x0, x1, x2` (w1 silently promoted to x1)

**Minimal failing input:** `encode_sbc(&[xreg(0), wreg(1), xreg(2)], false)`

## Impact

A typo or macro-generated mixed-width `SBC`/`SBCS` instruction assembles without error but performs an operation at the destination width, reading a different architectural register view than the source text names.

## Suggested Fix

Preserve and compare the `is_64` flags for all operands:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if rn64 != rd64 || rm64 != rd64 {
    return Err("SBC: all operands must share the destination's register width".to_string());
}
```

## Regression Property

Failing property: `sbc_rejects_mixed_width_operands`

```rust
prop_assert!(encode_sbc(&[xreg(0), wreg(1), xreg(2)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/92