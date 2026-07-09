# Bug Report: `encode_csel` silently accepts FP/SIMD registers and re-encodes them as GP registers

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csel`
**Severity:** Medium

## Summary

`encode_csel` validates only that operands 0–2 are `Operand::Reg`; it performs **no register-class check**. Because `get_reg` → `parse_reg_num` happily maps FP/SIMD register names (`b0`, `d1`, `s2`, `q3`, `h4`, `v5`) to a 5-bit number, any FP/SIMD register is silently accepted and emitted as if it were a general-purpose (X/W) register. The result is an AArch64 word that is **architecturally unallocated** for CSEL (and, depending on the exact field values, may alias a different instruction).

Per the ARM ARM (C4.1.64, *Conditional Select*), `CSEL` is defined **only** on general-purpose registers (`Rd`, `Rn`, `Rm` ∈ X/W). This also affects the entire conditional-select / compare family that shares `get_reg`.

## Root Cause

The encoder uses the generic `get_reg` helper which parses any register name (GP, FP, SIMD) and returns only the numeric register number without class information. No subsequent validation checks that the operands are GP registers.

## Reproduction

**Input:** `csel b0, x1, x2, eq`

**Expected:** `Err` — operand must be an integer register

**Actual:** `Ok(Word(0x9A080020))` — silently encoded as if `b0` were `x0`

**Minimal failing input:** `csel b0, x1, x2, eq`

## Impact

Silent mis-encoding produces architecturally unallocated instruction words. Hardware behavior is undefined (may trap, decode as a different instruction, or corrupt state). The lack of validation means FP/SIMD register typos assemble without error, producing wrong machine code.

## Suggested Fix

Add register-class validation before encoding:

```rust
let rd_str = match operands.get(0) { Some(Operand::Reg(r)) => r, _ => ... };
if is_fp_reg(rd_str) {
    return Err(format!("CSEL operand must be GP register, got {}", rd_str));
}
```

Apply the same check to `Rn` and `Rm`. Existing `is_fp_reg` helper matches prefixes `d | s | q | v | h | b`.

## Regression Property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_csel(&[Operand::Reg("b0".into()), xreg(1), xreg(2)], "eq").is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/28