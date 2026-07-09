# Bug Report: `encode_fsqrt` silently accepts GP-bank (non-FP) operands

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fsqrt`
**Severity:** Medium

## Summary

`encode_fsqrt` never checks operand bank. GP registers (`X`/`W`) parsed and encoded as if they were FP registers. FSQRT operates only on scalar FP registers (`S`/`D`); GP operands are architecturally UNALLOCATED.

## Root Cause

```rust
let is_double = rd_name.starts_with('d');          // GP 'x'/'w' => false => ftype 00
let ftype = if is_double { 0b01 } else { 0b00 };
// No bank validation
```

## Reproduction

**Input:** `fsqrt x0, x1`

**Expected:** `Err` — fsqrt requires FP-register operands

**Actual:** `Ok(Word(0x1E21C000))` — emits `FSQRT S0, S1` (GP register numbers treated as FP)

**Minimal failing input:** rd="x0", rn="x1"

## Impact

Nonsensical `FSQRT X0, X1` assembled without diagnostic, producing wrong runtime behavior. Same defect class as `encode_fneg`/`encode_fabs`.

## Suggested Fix

Require FP-register operands:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
if !is_fp_reg(&rd_name) || !is_fp_reg(&rn_name) {
    return Err("fsqrt requires FP-register operands".to_string());
}
```

## Regression Property

Failing property: `prop_fsqrt_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fsqrt(&[xreg(0), xreg(1)]).is_err());  // GP bank
prop_assert!(encode_fsqrt(&[wreg(0), wreg(1)]).is_err());  // GP bank
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/173