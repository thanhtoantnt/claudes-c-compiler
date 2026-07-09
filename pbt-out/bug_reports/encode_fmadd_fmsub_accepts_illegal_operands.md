# Bug Report: `encode_fmadd_fmsub` silently accepts illegal operands (no precision/bank validation)

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fmadd_fmsub`
**Severity:** High

## Summary

`encode_fmadd_fmsub` derives `ftype` (precision) field **solely from destination (`operands[0]`)**, never validating: (1) operands are FP registers (GP `Wn`/`Xn` silently accepted), (2) all four FP operands share same precision (`S` vs `D`). ARMv8-A requires homogeneous precision across `Rd, Rn, Rm, Ra`.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
// No prefix check on operands[1..4], no is_fp_reg check
```

## Reproduction

**Input:** `fmadd d0, s0, s0, s0`

**Expected:** `Err` — fmadd/fmsub operands must all be the same precision (all S or all D)

**Actual:** `Ok(Word(0x1F400000))` — `FMADD D0, D0, D0, D0` (single sources silently treated as double)

**Other failing input:** `fmadd x0, x1, x2, x3` → GP bank accepted, encoded as single-precision

## Impact

Miscompilation: emitted word doesn't match textual instruction. GP registers accepted and encoded as FP. Obvious malformed input produces success instead of error.

## Suggested Fix

Validate all operands are FP registers with homogeneous precision:

```rust
let prefixes = (0..4).map(|i| match &operands[i] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("expected register".into()),
}).collect::<Vec<_>>();
if !prefixes.iter().all(|n| is_fp_reg(n)) {
    return Err("fmadd/fmsub requires floating-point register operands".to_string());
}
let is_double = prefixes.iter().all(|n| n.starts_with('d'));
let all_single = prefixes.iter().all(|n| n.starts_with('s'));
if !(is_double || all_single) {
    return Err("fmadd/fmsub operands must all be the same precision (all S or all D)".to_string());
}
```

## Regression Property

Failing property: `prop_fmadd_rejects_mixed_precision_and_gp_bank`

```rust
prop_assert!(encode_fmadd_fmsub(&[dreg(0), sreg(0), sreg(0), sreg(0)], false).is_err());  // mixed precision
prop_assert!(encode_fmadd_fmsub(&[xreg(0), xreg(1), xreg(2), xreg(3)], false).is_err());  // GP bank
```

## PBT Results (module `tests::fmadd_fmsub`)

| Property | Result |
|---|---|
| `prop_fmadd_places_fields` | PASS |
| `prop_fmadd_ftype_and_o1_derivation` | PASS |
| `prop_fmadd_is_deterministic` | PASS |
| `prop_fmadd_rejects_bad_regs_arity` | PASS |
| `prop_fmadd_rejects_mixed_precision_and_gp_bank` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/171