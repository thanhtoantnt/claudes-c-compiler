# Bug Report: `encode_fabs` does not validate operand bank or precision homogeneity

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fabs`
**Severity:** Medium

## Summary

`encode_fabs` derives `ftype` precision field **only from destination (`operands[0]`)**, never validating: (1) both operands are FP-register operands (not GP `x`/`w`), (2) source precision **matches** destination. ARMv8-A requires homogeneous FP-register operands (`FABS Sd,Sn` or `FABS Dd,Dn`).

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');          // <-- dest-only
let ftype = if is_double { 0b01 } else { 0b00 };   // <-- dest-only
// No check for source bank/precision match
```

## Reproduction

**Input:** `fabs d0, s0`

**Expected:** `Err` — FABS operands must have matching precision

**Actual:** `Ok(Word(0x1E60C000))` — `FABS D0,D0` (single→double mismatch silently lost)

**Other failing input:** `fabs x0, x0` → encodes as `FABS S0,S0` (GP→FP silent coercion)

## Impact

Typo `FABS D0,S0` returns success, producing semantically wrong machine code. GP operands silently encoded instead of rejected.

## Suggested Fix

Validate homogeneous FP precision and reject non-FP banks:

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
let rd_is_fp = matches!(rd_name.chars().next(), Some('s') | Some('d') | Some('h'));
let rn_is_fp = matches!(rn_name.chars().next(), Some('s') | Some('d') | Some('h'));
if !rd_is_fp || !rn_is_fp || rd_name.starts_with('d') != rn_name.starts_with('d') {
    return Err("FABS requires FP operands of matching precision".into());
}
```

## Regression Property

Failing property: `prop_fabs_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fabs(&[dreg(0), sreg(0)]).is_err());  // mismatched precision
prop_assert!(encode_fabs(&[xreg(0), xreg(0)]).is_err());  // GP bank
prop_assert!(encode_fabs(&[sreg(0), xreg(0)]).is_err());  // mixed bank
```

## PBT Results

| Property | Result |
|---|---|
| `prop_fabs_places_fields` | PASS |
| `prop_fabs_ftype_from_dest` | PASS |
| `prop_fabs_is_deterministic` | PASS |
| `prop_fabs_rejects_out_of_range_reg` | PASS |
| `prop_fabs_rejects_mismatched_precision_and_bank` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/170