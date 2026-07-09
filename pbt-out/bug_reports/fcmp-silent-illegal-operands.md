# Bug Report: `encode_fcmp` silently accepts illegal operands (mixed precision & GP bank)

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fcmp`
**Severity:** High

## Summary

`encode_fcmp` derives the precision field `ftype` **only** from `operand[0]`'s name prefix and never validates `operand[1]`. Two classes of illegal operands are accepted without error:

1. **Mixed precision**: `FCMP D0, S1` → encodes as `FCMP D0, D1` (silently re-types)
2. **GP bank operands**: `FCMP W0, W1` → encodes as `FCMP S0, S1` (GP→FP silent bank mismatch)

## Root Cause

```rust
let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rn_name.starts_with('d');                 // <-- only operand[0]
let ftype = if is_double { 0b01 } else { 0b00 };
...
let (rm, _) = get_reg(operands, 1)?;                      // <-- no bank/precision check
```

## Reproduction

**Input:** `FCMP D0, S1`

**Expected:** `Err` — FCMP requires same-precision FP operands

**Actual:** `Ok(Word(0x1E612000))` — encodes as `FCMP D0, D1` (silently re-typed S1→D1)

**Minimal failing input:** `FCMP D0, S1` or `FCMP W0, W1`

## Impact

Silent mis-encoding: produces architecturally invalid instructions that don't match source text. Affects all mixed-precision and GP-bank operand combinations.

## Suggested Fix

Validate both operands share same FP prefix and reject non-FP bank:

```rust
let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
let rm_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
let rn_is_fp = rn_name.starts_with('s') || rn_name.starts_with('d');
let rm_is_fp = rm_name.starts_with('s') || rm_name.starts_with('d');
if !rn_is_fp || !rm_is_fp || rn_name.starts_with('d') != rm_name.starts_with('d') {
    return Err(format!("FCMP requires same-precision FP operands, got {}, {}", rn_name, rm_name));
}
```

## Regression Property

Failing property: `prop_fcmp_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fcmp(&[dreg(0), sreg(1)]).is_err());  // mixed precision
prop_assert!(encode_fcmp(&[wreg(0), wreg(1)]).is_err());  // GP bank
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/131