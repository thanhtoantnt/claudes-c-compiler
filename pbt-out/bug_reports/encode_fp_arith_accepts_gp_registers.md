# Bug: `encode_fp_arith` accepts non-FP (GP) registers — no operand-bank validation

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fp_arith(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String>`
**Severity:** Medium (latent — the parser/front-end currently feeds FP operands, but the encoder has no defense-in-depth.)

## Summary
`encode_fp_arith` never calls `is_fp_reg`. Any register name that does not start
with `'d'` is silently treated as single-precision. A general-purpose register
like `x0`/`w0`, or a SIMD register `q0`/`v0`, is therefore accepted and encoded
as if it were an FP operand, producing an **UNALLOCATED / UNDEFINED** AArch64
instruction that real hardware traps on. `as`/`llvm-mc` reject
`FADD X0, X1, X2` with "invalid operand combination".

## Minimal failing input
```
encode_fp_arith(&[Reg("x0"), Reg("x0"), Reg("x0")], 0b0010)  // FADD X0,X0,X0
```
- **Expected:** `Err` (GP registers are not valid FP operands).
- **Actual:** `Ok(Word(0x1E200800))` — `ftype = 0b00` because `x0` doesn't start with `'d'`.

Discovered by property `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`
(minimal input `n = 0`).

## Impact
The encoder can emit words the target CPU rejects (Unallocated Instruction
syndrome). Any future caller or hand-written-asm path passing GP/SIMD registers
gets `Ok(Word(...))` instead of a diagnostic, yielding invalid codegen silently.

## Fix
Validate the bank of all three operands before encoding:

```rust
for (i, op) in operands.iter().take(3).enumerate() {
    if let Operand::Reg(name) = op {
        if !is_fp_reg(name) {
            return Err(format!("fp_arith operand {} ({}) is not an FP register", i, name));
        }
    }
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/146
