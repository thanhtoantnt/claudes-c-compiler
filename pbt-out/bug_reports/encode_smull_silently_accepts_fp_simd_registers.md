# Bug Report: `encode_smull` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smull`
**Severity:** Medium

## Summary

`SMULL <Xd>,<Wn>,<Wm>` is a general-purpose (integer) widening multiply. Its
operands must come from the GP register file (`x`/`w`). An FP/SIMD register name
(`d`/`s`/`q`/`v`/`h`/`b`) is the wrong register **file** and is unallocated.
`encode_smull` resolves operands via the shared `parse_reg_num`, which accepts
every prefix, so e.g. `smull d0, w1, w2` is silently encoded bit-identically to
`smull x0, w1, w2` (it reads GP `x0`, not SIMD `d0`).

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    match prefix { 'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => { ... } }   // no file check
}
```

`encode_smull` performs no register-class validation on top of `parse_reg_num`.

## Reproduction

**Input:** `smull d0, w1, w2`
**Expected:** `Err` — FP/SIMD operand not permitted for SMULL
**Actual:** `Ok(Word(...))` — bit-identical to `smull x0, w1, w2`
**Minimal failing input (PBT-shrunk):** `encode_smull(&[Operand::Reg("d0".into()), wreg(1), wreg(2)])`

Differential oracle: `echo 'smull d0, w1, w2' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-assembly: a SIMD register name is accepted where only a GP register is
meaningful, producing an instruction that operates on the wrong register file
with no diagnostic.

## Suggested Fix

Reject FP/SIMD prefixes (and SP/WSP — see sibling report) in every operand
position before encoding, using a shared GP-only operand validator.

## Regression Property

Failing witness: `smull_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::smull_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/339
