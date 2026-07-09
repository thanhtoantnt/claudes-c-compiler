# Bug Report: `encode_smulh` accepts 32-bit (W) register operands without validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smulh`
**Severity:** Medium

## Summary

`encode_smulh` calls `get_reg(operands, i)` for each operand, which returns `(reg_num, is_64)` but **discards the `is_64` flag**. As a result, `smulh w0, w1, w2` is silently encoded as a 64-bit instruction (`sf = 1`).

## Specification

Per the ARMv8 ARM, **SMULH** (Signed Multiply High) is defined **only** as `Xd, Xn, Xm` (all 64-bit). There is **no 32-bit (W) form**. Assemblers reject W-register operands.

## Root Cause

```rust
pub(crate) fn encode_smulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    let word = (1u32 << 31) | ...;          // sf hardcoded to 1, no width check
    Ok(EncodeResult::Word(word))
}
```

## Reproduction

**Input:** `smulh w0, w1, w2`

**Expected:** `Err` — SMULH requires 64-bit (X) registers

**Actual:** `Ok(Word(0x9b407c00))` — silently encodes as `smulh x0, x1, x2`

## Impact

Silent mis-assembly: W-register operands accepted and mis-encoded as 64-bit. Same pattern affects `UMULH`, `UMULL`, `SMLH`, `SMADDL`, `UMADDL`.

## Suggested Fix

Validate all operands are 64-bit:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !rd64 || !rn64 || !rm64 {
    return Err("smulh requires 64-bit (X) registers".to_string());
}
```

## Regression Property

Failing property: `smulh_rejects_32bit_w_registers`

```rust
prop_assert!(encode_smulh(&[wreg(0), wreg(1), wreg(2)]).is_err());  // W operands
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/122