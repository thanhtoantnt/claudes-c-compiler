# Bug Report: `encode_cset` silently accepts FP/SIMD destination registers

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_cset`

## Summary

`encode_cset` uses the shared generic register parser, which accepts FP/SIMD register names (`d`, `s`, `q`, `v`, `h`, `b`) and returns only their numeric index. CSET is a GP-register instruction, so FP/SIMD destination operands must be rejected. Instead, the encoder silently reinterprets the numeric index as a GP register.

## Reproduction

Failing property: `prop_rejects_fp_simd_registers`

Minimal input from the run:

```text
cset d0, eq
```

Actual behavior: returns `Ok(Word(_))` instead of `Err`.

`llvm-mc-18` rejects the same input with an invalid-operand diagnostic.

## Impact

Invalid FP/SIMD-register source is accepted and assembled into a GP-register instruction with the same numeric register index, emitting an instruction the programmer did not write.

## Suggested fix

Use a GP-only register parser for CSET and the conditional-select alias family:

```rust
fn get_gp_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    let (reg, is_64) = get_reg(operands, idx)?;
    let name = get_reg_name(operands, idx)?;
    if is_fp_reg(&name) {
        return Err(format!("expected integer register, got {}", name));
    }
    Ok((reg, is_64))
}
```
