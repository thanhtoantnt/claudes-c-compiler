# Bug Report: `encode_msub` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_msub`
**Severity:** High

## Summary

`encode_msub` resolves operands through `get_reg`/`parse_reg_num`, which accept
FP/SIMD register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`). MSUB (Data-processing
(3 source), `o0=1`) operates only on **general-purpose** registers; a SIMD name in
any of `Rd`/`Rn`/`Rm`/`Ra` is a register-class violation the reference assembler
rejects. The encoder accepts it and emits an instruction bit-identical to the
same-numbered GP register.

For example `msub x0, x1, v2, x3` encodes identically to `msub x0, x1, x2, x3`.

## Root Cause

Same as `encode_mul`/`encode_madd`: `parse_reg_num` lumps FP/SIMD prefixes with
GP, and `encode_msub` performs no register-class check.

## Reproduction

**Input:** `msub x0, x0, v0, x0`
**Expected:** `Err` — `v0` is a SIMD register, not a GP operand
**Actual:** `Ok(Word(...))` — bit-identical to `msub x0, x0, x0, x0`
**Minimal failing input:** `encode_msub(&[Operand::Reg("x0".into()), Operand::Reg("x0".into()), Operand::Reg("v0".into()), Operand::Reg("x0".into())])`

Differential check: `echo 'msub x0, x0, v0, x0' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a SIMD-named operand is read/written as the same-numbered
GP register with no diagnostic.

## Suggested Fix

Apply a shared `reject_fp_simd` check across all four operand positions before
encoding.

## Regression Property

Failing witness: `msub_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_mul_madd_msub_umaddl_umull_pbt::msub_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)
