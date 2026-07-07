# Bug Report: `encode_madd` silently accepts mixed-width register operands

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_madd`

## Summary

`encode_madd` derives the instruction width (`sf`) only from the destination register (`Rd`) and discards the width class of `Rn`, `Rm`, and `Ra`. Mixed W/X operands are silently accepted and encoded as either the 32-bit or 64-bit form based only on `Rd`.

## Reproduction

Characterization from the PBT campaign:

```text
madd x0, w1, x2, x3
```

Actual result:

```text
Ok(Word(0x9b028060))
```

The encoding has `sf=1` from `x0`, while `w1` was a 32-bit operand. AArch64 MADD operands must have consistent width.

## Impact

Invalid mixed-width multiply-add source is accepted and silently assembled to a different-width instruction, producing undefined/unpredictable semantics instead of a diagnostic.

## Suggested fix

Parse all four register operands with width information and reject mismatches:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
if rn64 != rd64 || rm64 != rd64 || ra64 != rd64 {
    return Err("madd register operands must have matching widths".to_string());
}
```
