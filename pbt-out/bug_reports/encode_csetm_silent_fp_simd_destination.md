# Bug Report: `encode_csetm` silently accepts FP/SIMD destination registers

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_csetm`

## Summary

`encode_csetm` uses the shared generic register parser, which accepts FP/SIMD register names (`d`, `s`, `q`, `v`, `h`, `b`) and returns only their numeric index. CSETM is a GP-register instruction (alias of `CSINV <Rd>, <XZR>, <XZR>, invert(<cond>)`, ARM ARM C6.2.44), so FP/SIMD destination operands must be rejected. Instead, the encoder silently reinterprets the numeric index as a GP register and emits a malformed instruction.

## Reproduction

Failing property: `prop_rejects_fp_simd_registers` (in `prop_encode_csetm_tests`).

Minimal input from the run:

```text
prefix = "b", n = 0
```

i.e.:

```rust
let ops = vec![Operand::Reg("b0".into()), Operand::Cond("eq".into())];
encode_csetm(&ops)   // returns Ok(Word(1520374752))
```

Actual behavior: returns `Ok(Word(0x5A9F13E0))` instead of `Err`.

Decoding the emitted word:

```
0x5A9F13E0 == sf(0) op(1) S(0) 11010100 Rm(=11111) cond(=1=ne=invert(eq))
                 o2(0) o1(0) Rn(=11111) Rd(=0)
```

`Rd=0` came straight from `parse_reg_num("b0") == Some(0)`. The FP/SIMD register
prefix was silently discarded and its index used as if it were `x0`/`w0`, and
because `is_64bit_reg("b0") == false`, `sf` was forced to 0 (32-bit / W form).
The condition handling (`eq` -> invert -> `ne` -> field 1) is correct here; the
corruption is purely in the register class and width. A reference assembler
(`llvm-mc`) rejects `csetm b0, eq` with an invalid-operand diagnostic.

## Impact

Invalid FP/SIMD destination operand is accepted and assembled into a
GP-register instruction with the same numeric register index, emitting an
instruction the programmer did not write and that operates on the wrong
register file. Because `sf` is derived from `is_64bit_reg` (which is false for
every FP/SIMD prefix), the emitted word is silently forced to the 32-bit (W)
form as well, corrupting both the register *class* and the instruction width.

## Root cause

`encode_csetm` resolves `Rd` through `get_reg` (in `encoder/mod.rs`), which
calls `parse_reg_num`. That matcher treats every FP/SIMD prefix as valid and
extracts only the numeric index:

```rust
fn parse_reg_num(name: &str) -> Option<u32> {
    // matches d/s/q/v/h/b prefixes and returns the trailing number
    ...
}
```

No call site in `encode_csetm` checks that `Rd` is actually a general-purpose
(X/W) register. The same gap affects the whole conditional-select alias family
(`encode_csel`, `encode_csinc`, `encode_csinv`, `encode_csneg`, `encode_cset`,
`encode_csetm`); see the cross-cutting report
`encode_csel_silently_accepts_fp_simd_registers.md`. This is the dedicated
report for `encode_csetm`.

## Suggested fix

Use a GP-only register parser for the CSETM destination:

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

and route `encode_csetm`'s `Rd` through it.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/32
