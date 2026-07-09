# Bug Report: `encode_fsqrt` silently accepts mixed-precision operands

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fsqrt`
**Severity:** Medium

## Summary

`encode_fsqrt` derives `ftype` from destination only. Source operand consumed only for register number, never compared for precision. Mixed-precision `FSQRT D0, S0` accepted, re-encoded as `FSQRT D0, D0`.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');          // ONLY dest inspected
let ftype = if is_double { 0b01 } else { 0b00 };
// No source precision check
```

## Reproduction

**Input:** `fsqrt d0, s0`

**Expected:** `Err` — fsqrt operands must have matching precision

**Actual:** `Ok(Word(0x1E61C000))` — `FSQRT D0, D0` (S0 silently re-encoded as D)

**Minimal failing input:** rd="d0", rn="s0"

## Impact

Malformed `FSQRT D0, S0` accepted, assembled into semantically different instruction. No diagnostic, program silently computes wrong value. Same defect as `encode_fneg`/`encode_fabs`.

## Suggested Fix

Validate matching precision:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let rd_double = rd_name.starts_with('d');
let rn_double = rn_name.starts_with('d');
if rd_double != rn_double {
    return Err("fsqrt operands must have matching precision".to_string());
}
```

## Regression Property

Failing property: `prop_fsqrt_rejects_mismatched_precision_and_bank`

```rust
prop_assert!(encode_fsqrt(&[dreg(0), sreg(0)]).is_err());  // D, S mismatch
prop_assert!(encode_fsqrt(&[sreg(0), dreg(0)]).is_err());  // S, D mismatch
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/174