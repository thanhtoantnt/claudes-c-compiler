# Bug Report: `encode_fcmp` does not validate the second operand's precision/bank

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fcmp`
**Severity:** High

## Summary

`encode_fcmp` derives `ftype` (precision) field from `operands[0]` only, never inspecting `operands[1]`. Mixed-precision operand `FCMP D0, S0` rewritten to `FCMP D0, D0`. ARMv8 ARM requires homogeneous-precision FP-register operands.

## Root Cause

```rust
let is_double = rn_name.starts_with('d');                 // only operand[0]
let ftype = if is_double { 0b01 } else { 0b00 };
let (rm, _) = get_reg(operands, 1)?;                      // operands[1] prefix never checked
```

## Reproduction

**Input:** `fcmp d0, s0`

**Expected:** `Err` — fcmp operands must have matching precision

**Actual:** `Ok(Word(0x1E602000))` — decodes to `FCMP D0, D0` (S0 silently dropped)

**Other failing inputs:** `fcmp s0, d0` → `FCMP S0, S0`; `fcmp x0, x0` → GP bank accepted

## Impact

Silent miscompile: operand dropped/rewritten. `FCMP X0, X0` (GP bank) accepted and coerced. No diagnostic for illegal operands.

## Suggested Fix

Validate both operands are FP-bank and share precision:

```rust
let rm_name = match &operands[1] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("fcmp: expected register operand".into()),
};
let rm_is_fp = matches!(rm_name.chars().next(), Some('s') | Some('d'));
if !rm_is_fp || is_double != rm_name.starts_with('d') {
    return Err("fcmp requires homogeneous-precision FP operands".into());
}
```

## Regression Property

Failing property: `prop_fcmp_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fcmp(&[dreg(0), sreg(0)]).is_err());  // mixed precision
prop_assert!(encode_fcmp(&[sreg(0), dreg(0)]).is_err());  // mixed precision
prop_assert!(encode_fcmp(&[xreg(0), xreg(0)]).is_err());  // GP bank
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/172