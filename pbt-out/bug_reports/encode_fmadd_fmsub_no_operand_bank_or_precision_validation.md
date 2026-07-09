# Bug Report: `encode_fmadd_fmsub` does not validate operand banks or precision

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fmadd_fmsub`
**Severity:** High

## Summary

`encode_fmadd_fmsub` derives `ftype` **only from destination**, validating neither FP register bank nor precision homogeneity. ARMv8-A requires all four FP/SIMD registers to share single precision (`all S` or all D`). GP-bank operands accepted, producing mis-encoded instructions.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };   // dest-only derivation
// No is_fp_reg() check, no precision coherence check
```

## Reproduction

**Input:** `FMADD D0, S0, S0, S0`

**Expected:** `Err` — FMADD/FMSUB requires all operands to be same FP precision

**Actual:** `Ok(Word(0x1F400000))` — encoded as FMADD D0, D0, D0, D0 (S-register silently promoted to D)

**Other failing input:** `FMADD X0, X0, X0, X0` → `0x1F000000` (GP bank accepted, same as FMADD S0,S0,S0,S0)

## Impact

Mixed-precision forms accepted silently. GP-bank operands collide with FP encodings. LLVM rejects these inputs.

## Suggested Fix

Validate operand banks and precision homogeneity:

```rust
let names: Vec<_> = (0..4).map(|i| match &operands[i] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("FMADD/FMSUB requires register operands".into()),
}).collect();
let is_double = names[0].starts_with('d');
if !names.iter().all(|n| n.starts_with('d') == is_double) {
    return Err("FMADD/FMSUB requires homogeneous FP precision (all S or all D)".into());
}
```

## Regression Property

Failing property: `prop_fmadd_rejects_mixed_precision_and_gp_bank`

```rust
prop_assert!(encode_fmadd_fmsub(&[dreg(0), sreg(1), sreg(2), sreg(3)], false).is_err());  // mixed precision
prop_assert!(encode_fmadd_fmsub(&[xreg(0), xreg(1), xreg(2), xreg(3)], false).is_err());  // GP bank
prop_assert!(encode_fmadd_fmsub(&[wreg(0), wreg(1), wreg(2), wreg(3)], true).is_err());   // GP bank (FMSUB)
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/224