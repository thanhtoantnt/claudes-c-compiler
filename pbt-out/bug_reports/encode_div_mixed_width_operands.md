# Bug Report: `encode_div` silently accepts mixed-width operands

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_div`

## Summary

`encode_div` derives the instruction width (`sf`) only from the destination register (`Rd`) and discards the width class of `Rn` and `Rm`. Mixed W/X operands are silently accepted and encoded using the destination width.

## Reproduction

Characterization from the PBT campaign:

```text
sdiv x0, w1, x2
```

Actual behavior: returns `Ok(Word(_))` with `sf=1` from `x0`, despite `w1` being a 32-bit operand.

## Impact

Invalid mixed-width division source is accepted and assembled to an undefined/unpredictable instruction instead of producing a diagnostic.

## Suggested fix

Parse width for all operands and reject mismatches:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if rn64 != rd64 || rm64 != rd64 {
    return Err("div register operands must have matching widths".to_string());
}
```

## Regression property

Failing property: `div_mixed_width_operands_rejected`

```rust
prop_assert!(encode_sdiv(&[xreg(0), wreg(1), xreg(2)]).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/36
