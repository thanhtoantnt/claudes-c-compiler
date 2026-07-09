# Bug Report: `encode_fp_1src`: no FP-bank / precision-homogeneity validation

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fp_1src`
**Severity:** Medium

## Summary

`ftype` derived **only from destination register prefix**. Never verifies: (1) source register matches destination's precision, (2) operands are FP registers. ARMv8-A FRINT* require homogeneous S/D (or H under FP16) FP-register operands.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
// No source validation, no FP-bank check
```

## Reproduction

**Input:** `frintn d0, s1`

**Expected:** `Err` — FP 1-source operands must have matching precision

**Actual:** `Ok(Word(...))` — `ftype=01` taken from dest only, source silently mismatched

**Other failing inputs:** `frintn x0, x1` (GP registers accepted), `frintn h0, h1` (half-precision silently treated as single)

## Impact

Wrong code on malformed operands. GP registers accepted and encoded as FP. Half-precision silently mapped to single.

## Suggested Fix

Validate both operands are FP registers of same precision:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
if !is_fp_reg(&rd_name) || !is_fp_reg(&rn_name) {
    return Err("FP 1-source instructions require FP register operands".into());
}
if rd_name.starts_with('d') != rn_name.starts_with('d') {
    return Err("FP 1-source operands must have matching precision".into());
}
```

## Regression Property

Failing property: `prop_fp_1src_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fp_1src(&[dreg(0), sreg(1)], 0b000001).is_err());  // mismatched precision
prop_assert!(encode_fp_1src(&[xreg(0), xreg(1)], 0b000001).is_err());  // GP bank
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/128