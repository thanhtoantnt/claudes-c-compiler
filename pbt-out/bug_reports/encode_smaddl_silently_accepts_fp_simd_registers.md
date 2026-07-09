# Bug Report: `encode_smaddl` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smaddl`
**Severity:** Medium

## Summary

`SMADDL <Xd>,<Wn>,<Wm>,<Xa>` is a general-purpose (integer) widening
multiply-accumulate; its operands must come from the GP register file. An FP/SIMD
register name (`d`/`s`/`q`/`v`/`h`/`b`) is the wrong register **file** and is
unallocated. `encode_smaddl` resolves operands via the shared `parse_reg_num`,
which accepts every prefix, so e.g. `smaddl x0, v1, w2, x3` is silently encoded
bit-identically to `smaddl x0, x1, w2, x3` (reads GP `x1`, not SIMD `v1`).

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    match prefix { 'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => { ... } }   // no file check
}
```

`encode_smaddl` performs no register-class validation on top of `parse_reg_num`.

## Reproduction

**Input:** `smaddl x0, v1, w2, x3`
**Expected:** `Err` — FP/SIMD operand not permitted for SMADDL
**Actual:** `Ok(Word(...))` — bit-identical to `smaddl x0, x1, w2, x3`
**Minimal failing input (PBT-shrunk):** `encode_smaddl(&[Operand::Reg("d0".into()), wreg(0), wreg(0), xreg(0)])`

Differential oracle: `echo 'smaddl x0, v1, w2, x3' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-assembly: a SIMD register name is accepted where only a GP register is
meaningful, operating on the wrong register file with no diagnostic.

## Suggested Fix

Reject FP/SIMD prefixes (and SP/WSP — see sibling report) in every operand
position before encoding, using a shared GP-only operand validator.

## Regression Property

Failing witness: `smaddl_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::smaddl_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)
