# Bug Report: `encode_mneg` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mneg`
**Severity:** Medium

## Summary

`MNEG <Rd>,<Rn>,<Rm>` is a general-purpose (integer) multiply-negate (alias of
`MSUB Rd,Rn,Rm,XZR`); its operands must come from the GP register file. An
FP/SIMD register name (`d`/`s`/`q`/`v`/`h`/`b`) is the wrong register **file**
and is unallocated. `encode_mneg` resolves operands via the shared `parse_reg_num`,
which accepts every prefix, so e.g. `mneg x0, d1, x2` is silently encoded
bit-identically to `mneg x0, x1, x2` (reads GP `x1`, not SIMD `d1`).

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    match prefix { 'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => { ... } }   // no file check
}
```

`encode_mneg` performs no register-class validation on top of `parse_reg_num`.

## Reproduction

**Input:** `mneg x0, d1, x2`
**Expected:** `Err` — FP/SIMD operand not permitted for MNEG
**Actual:** `Ok(Word(...))` — bit-identical to `mneg x0, x1, x2`
**Minimal failing input (PBT-shrunk):** `encode_mneg(&[Operand::Reg("d0".into()), xreg(0), xreg(0)])`

Differential oracle: `echo 'mneg d0, x1, x2' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-assembly: a SIMD register name is accepted where only a GP register is
meaningful, operating on the wrong register file with no diagnostic.

## Suggested Fix

Reject FP/SIMD prefixes (and SP/WSP — see sibling report) in every operand
position before encoding, using a shared GP-only operand validator.

## Regression Property

Failing witness: `mneg_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::mneg_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/333
