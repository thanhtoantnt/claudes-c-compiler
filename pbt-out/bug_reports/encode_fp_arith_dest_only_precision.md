# Bug Report: `encode_fp_arith` derives `ftype` from destination only — no precision-consistency check

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fp_arith`
**Severity:** Medium

## Summary

`ftype` read **only from destination (`operands[0]`)** first character. Source operands (`Rn`, `Rm`) precision never inspected, no consistency enforced. AArch64 scalar FP data-processing (2-source) requires destination and both sources share precision; mixed-precision word is UNALLOCATED.

## Root Cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01 } else { 0b00 };   // dest-only
// No source precision check
```

## Reproduction

**Input:** `fadd d0, s1, s2`

**Expected:** `Err` — fp_arith operands must share precision (all D or all S)

**Actual:** `Ok(Word(0x1E608820))` — `ftype=01` from D0, sources silently reinterpreted as double

**Minimal failing input:** rd="d0", rn="s1", rm="s2"

## Impact

Mismatched-precision operands yield UNALLOCATED encodings that real AArch64 hardware rejects, with no compiler diagnostic.

## Suggested Fix

Require homogeneous precision across all three FP operands:

```rust
let prec: Vec<char> = operands.iter().take(3).filter_map(|o| match o {
    Operand::Reg(n) => n.to_lowercase().chars().next(),
    _ => None,
}).collect();
if !(prec.iter().all(|&c| c == 'd') || prec.iter().all(|&c| c == 's')) {
    return Err("fp_arith operands must share precision (all D or all S)".into());
}
```

## Regression Property

Failing property: `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`

```rust
prop_assert!(encode_fp_arith(&[dreg(0), sreg(1), sreg(2)], 0b0010).is_err());  // mixed precision
prop_assert!(encode_fp_arith(&[sreg(0), dreg(1), dreg(2)], 0b0010).is_err());  // mixed precision
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/147