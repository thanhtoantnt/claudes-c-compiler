# Bug Report: `encode_cas` silently accepts register-width violations

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_cas`
**Severity:** Medium

## Summary

`encode_cas` derives `size` field **only** from `Rs` register (operand 0) and mnemonic suffix. Width of `Rt` (operand 1) is parsed and discarded. ARMv8-A requires: (1) `Rs` and `Rt` must be same width, (2) `CASB`/`CASH` must use W registers only.

## Root Cause

```rust
let (rs, is_64) = get_reg(operands, 0)?;   // is_64 used for size
let (rt, _) = get_reg(operands, 1)?;       // <-- is_64 thrown away
```

## Reproduction

**Input:** `casb x0, x1, [x2]`

**Expected:** `Err` — byte CAS requires W registers

**Actual:** `Ok(Word(0x08A07C41))` — identical to `casb w0, w1, [x2]`

**Other failing inputs:** `cash x0,x1,[x2]`, `cas w0,x1,[x2]`, `cas x0,w1,[x2]`

## Impact

UNDEFINED operand combinations accepted. `Rs` width determines `size`, `Rt` width ignored. `casb x0,x1` encodes as `size=00` from `w0`, masking invalid X registers.

## Suggested Fix

Capture and validate `Rt` width:

```rust
let (rs, rs_is_64) = get_reg(operands, 0)?;
let (rt, rt_is_64) = get_reg(operands, 1)?;
if suffix_size.is_none() && rs_is_64 != rt_is_64 {
    return Err(format!("{}: Rs and Rt must have the same width", mnemonic));
}
if suffix_size.is_some() && (rs_is_64 || rt_is_64) {
    return Err(format!("{}: byte/half CAS requires W registers", mnemonic));
}
```

## Regression Property

Failing property: `prop_width_violation_rejected`

```rust
prop_assert!(encode_cas("casb", &[xreg(0), xreg(1), mem(xreg(2))]).is_err());  // byte with X
prop_assert!(encode_cas("cas", &[wreg(0), xreg(1), mem(xreg(2))]).is_err());   // mismatched
```

## PBT Results (module `prop_encode_cas_tests`)

| Property | Result |
|---|---|
| Fixed bits / field placement | PASS |
| Acquire/release differential | PASS |
| Size-from-suffix/width | PASS |
| Operand-count/range negative | PASS |
| `prop_width_violation_rejected` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/139