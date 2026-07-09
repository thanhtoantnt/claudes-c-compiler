# Bug Report: `encode_fp_arith` accepts non-FP (GP) registers — no operand-bank validation

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fp_arith`
**Severity:** Medium

## Summary

`encode_fp_arith` never calls `is_fp_reg`. Any register name that does not start with `'d'` is silently treated as single-precision. A general-purpose register like `x0`/`w0`, or a SIMD register `q0`/`v0`, is therefore accepted and encoded as if it were an FP operand, producing an **UNALLOCATED / UNDEFINED** AArch64 instruction that real hardware traps on. `as`/`llvm-mc` reject `FADD X0, X1, X2` with "invalid operand combination".

## Root Cause

The encoder derives the precision (`ftype`) only from the first character of the register name (`'d'` = double, anything else = single) without validating that the operand is actually an FP register. GP/SIMD registers pass this check silently and produce invalid encodings.

## Reproduction

**Input:** `fadd x0, x0, x0`

**Expected:** `Err` — GP registers are not valid FP operands

**Actual:** `Ok(Word(0x1E200800))` — silently encoded with `ftype = 0b00` (single-precision)

**Minimal failing input:** `encode_fp_arith(&[Reg("x0"), Reg("x0"), Reg("x0")], 0b0010)`

## Impact

The encoder can emit words the target CPU rejects (Unallocated Instruction syndrome). Any future caller or hand-written-asm path passing GP/SIMD registers gets `Ok(Word(...))` instead of a diagnostic, yielding invalid codegen silently. Real hardware will trap on these instructions.

## Suggested Fix

Validate that all operands are FP registers before encoding:

```rust
// After parsing Rd
if !is_fp_reg(rd_str) {
    return Err(format!("FP arithmetic requires FP registers, got {}", rd_str));
}
```

Apply the same check to `Rn` and `Rm`. Existing `is_fp_reg` helper matches prefixes `d | s` (and should be extended if other FP registers are valid for this instruction class).

## Regression Property

Failing property: `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`

```rust
prop_assert!(encode_fp_arith(&[xreg(0), xreg(0), xreg(0)], 0b0010).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/146