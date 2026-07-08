# Bug Report: `encode_umull` silently accepts mixed-width operands

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_umull`

## Summary

`encode_umull` uses the generic register parser and discards the width class of all operands. Mixed W/X spellings are silently accepted and encoded as a 64-bit UMULL/UMADDL form even when the source operand widths do not match the instruction spelling.

## Reproduction

Characterization from the PBT campaign:

```text
umull w0, w1, w2
```

The encoder returns `Ok(Word(_))` instead of rejecting the width mismatch.

## Impact

Invalid mixed-width source is accepted and assembled into an instruction that does not match the programmer's requested operand classes.

## Suggested fix

Track width for all three registers and reject mismatches:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !(rd64 && rn64 && rm64) {
    return Err("umull requires 64-bit destination and 32-bit sources".to_string());
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/110
