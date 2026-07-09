# Bug Report: `encode_umull` silently accepts mixed-width operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umull`
**Severity:** Medium

## Summary

`encode_umull` uses the generic register parser and discards the width class of all operands. Mixed W/X spellings are silently accepted and encoded as a 64-bit UMULL/UMADDL form even when the source operand widths do not match the instruction spelling.

## Root Cause

The encoder derives the `sf` bit only from the destination and does not validate that source operands match the required 32-bit form.

## Reproduction

**Input:** `umull w0, w1, w2`

**Expected:** `Err` — UMULL requires 64-bit destination and 32-bit sources

**Actual:** `Ok(Word(_))` — incorrectly encoded with wrong operand widths

**Minimal failing input:** `encode_umull(&[wreg(0), wreg(1), wreg(2)])`

## Impact

Invalid mixed-width source is accepted and assembled into an instruction that does not match the programmer's requested operand classes. The instruction may read the wrong register data or produce incorrect results.

## Suggested Fix

Track width for all three registers and reject mismatches:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !(rd64 && !rn64 && !rm64) {
    return Err("umull requires 64-bit destination and 32-bit sources".to_string());
}
```

## Regression Property

Failing property: `umull_rejects_mixed_width_operands`

```rust
prop_assert!(encode_umull(&[wreg(0), wreg(1), wreg(2)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/202