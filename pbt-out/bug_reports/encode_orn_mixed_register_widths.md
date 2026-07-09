# Bug Report: `encode_orn` silently accepts mixed X/W register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`
**Severity:** Medium

## Summary

All operands of a shifted-register logical op must share the same register width. GAS rejects `orn x0, w1, w2` with "Error: operand size mismatch". The encoder derives `sf` (bit 31) only from operand 0 and discards the widths of operands 1 and 2, so mixed X/W operands are silently encoded.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;
let (rm, _) = get_reg(operands, 2)?;
let sf = sf_bit(is_64);  // <-- only uses rd64
```

The `is_64` flags from operands 1 and 2 are discarded, so width mismatches are never checked.

## Reproduction

**Input:** `orn x0, w0, w0`

**Expected:** `Err` — operand size mismatch

**Actual:** `Ok` — encodes as 64-bit ORN reading W registers

**Minimal failing input:** rd = 0, rn = 0, rm = 0, mix = 0

## Impact

A typo'd `orn x0, w1, w2` assembles without error but produces a 64-bit instruction reading W registers, silent mis-compilation. Same defect exists in `encode_eon`, `encode_bics`, `encode_mvn`, `encode_logical`.

## Suggested Fix

Capture and compare the widths of all three register operands:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if rd64 != rn64 || rd64 != rm64 {
    return Err("orn operands must all be the same register width".to_string());
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/85