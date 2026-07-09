# Bug Report: `encode_smaddl` does not validate register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smaddl`
**Severity:** High

## Summary

`encode_smaddl` reads only register numbers, discarding width flags. ARMv8-A `SMADDL <Xd>, <Wn>, <Wm>, <Xa>` requires: `Xd` (64-bit), `Wn`/`Wm` (32-bit), `Xa` (64-bit). All invalid width combinations silently accepted.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded
```

## Reproduction

**Input:** `smaddl w0, w1, w2, w3`

**Expected:** `Err` — SMADDL requires Xd, Wn, Wm, Xa

**Actual:** `Ok(Word(...))` — wrong width combination accepted

**Other failing inputs:** `smaddl x0, x1, x2, x3` (sources wrong), `smaddl x0, w1, w2, w3` (accumulator wrong)

## Impact

All four invalid width combinations accepted, producing UNALLOCATED encodings without diagnostic.

## Suggested Fix

Validate widths per spec:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
if !rd64 {
    return Err("SMADDL: Rd must be 64-bit (X)".into());
}
if rn64 || rm64 {
    return Err("SMADDL: Rn and Rm must be 32-bit (W)".into());
}
if !ra64 {
    return Err("SMADDL: Ra must be 64-bit (X)".into());
}
```

## Regression Property

Failing property: `smaddl_rejects_width_violations`

```rust
prop_assert!(encode_smaddl(&[wreg(0), wreg(1), wreg(2), wreg(3)]).is_err());  // Rd wrong
prop_assert!(encode_smaddl(&[xreg(0), xreg(1), xreg(2), xreg(3)]).is_err());  // Rn/Rm wrong
prop_assert!(encode_smaddl(&[xreg(0), wreg(1), wreg(2), wreg(3)]).is_err());  // Ra wrong
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/100