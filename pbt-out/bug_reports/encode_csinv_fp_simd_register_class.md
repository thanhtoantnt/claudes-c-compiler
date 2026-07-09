# Bug Report: `encode_csinv` accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csinv`
**Severity:** Medium

## Summary

`encode_csinv` silently accepts FP/SIMD register names (`d`/`s`/`q`/`v`/`h`/`b`) in any slot, re-encoding with numeric index as if they were GP (X/W) registers. CSINV is defined ONLY on GP registers; FP/SIMD operands are architecturally invalid.

## Root Cause

`parse_reg_num` accepts every register-bank prefix (`x|w|d|s|q|v|h|b`). `encode_csinv` uses parsed number unconditionally, never validates GP class.

## Reproduction

**Input:** `csinv b0, x1, x2, eq`

**Expected:** `Err` — FP/SIMD register not valid for GP conditional-select

**Actual:** `Ok(Word(0x5A820020))` — b0 parsed as register 0, sf=0, GP CSINV emitted

**Minimal failing input:** prefix="b", n=0, slot=0

## Impact

GP instruction word emitted for FP/SIMD-sourced CSINV. Register class silently lost; output indistinguishable from legitimate GP encoding. Affects all three slots (Rd, Rn, Rm).

## Suggested Fix

Validate GP class for register operands:

```rust
let name = match &operands[idx] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("expected register".to_string()),
};
if !matches!(name.chars().next(), Some('w') | Some('x')) {
    return Err(format!("expected GP register, got {}", name));
}
```

## Regression Property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_csinv(&[breg(0), xreg(1), xreg(2), cond("eq")]).is_err());  // Rd
prop_assert!(encode_csinv(&[xreg(0), dreg(1), xreg(2), cond("eq")]).is_err());  // Rn
prop_assert!(encode_csinv(&[xreg(0), xreg(1), sreg(2), cond("eq")]).is_err());  // Rm
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/169