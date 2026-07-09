# Bug Report: `encode_fneg` silently accepts illegal operand types

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fneg`
**Severity:** High

## Summary

`encode_fneg` derives `ftype` from destination only and never validates bank. GP (W/X) operands and FP operands of wrong precision are accepted without error.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
// No bank validation, no precision match
```

## Reproduction

**Input:** `fneg x0, x1`

**Expected:** `Err` — FNEG requires FP/SIMD operands

**Actual:** `Ok(Word(0x1E214020))` — GP register numbers used as FP registers

**Other failing inputs:** `fneg d0, s1` (mixed precision) → encodes as `fneg d0, d1`

## Impact

GP-bank operands accepted and encoded as FP, producing silently wrong instructions. Mixed precision accepted, producing UNALLOCATED encodings.

## Suggested Fix

Validate bank before encoding:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
if !is_fp_reg(&rd_name) || !is_fp_reg(&rn_name) {
    return Err("FNEG requires FP/SIMD operands".into());
}
if rd_name.starts_with('d') != rn_name.starts_with('d') {
    return Err("FNEG operands must have matching precision".into());
}
```

## Regression Property

Failing property: `prop_fneg_rejects_gp_and_mixed`

```rust
prop_assert!(encode_fneg(&[xreg(0), xreg(1)]).is_err());  // GP bank
prop_assert!(encode_fneg(&[dreg(0), sreg(1)]).is_err());  // mixed precision
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/175