# Bug Report: `encode_umull` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umull`
**Severity:** High

## Summary

`encode_umull` (`UMULL <Xd>,<Wn>,<Wm>` = alias of `UMADDL <Xd>,<Wn>,<Wm>,XZR`)
resolves operands through `get_reg`/`parse_reg_num`, which accept FP/SIMD
register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`) and return the trailing lane
number as the register field. UMULL operates only on **general-purpose**
registers; a SIMD name in `Xd`/`Wn`/`Wm` is a register-class violation the
reference assembler rejects. The encoder accepts it and emits an instruction
bit-identical to the same-numbered GP register.

For example `umull x0, w0, v2` encodes identically to `umull x0, w0, w2`.

## Root Cause

Same as `encode_mul`: `parse_reg_num` lumps FP/SIMD prefixes with GP, and
`encode_umull` performs no register-class check before encoding.

## Reproduction

**Input:** `umull x0, w0, v0`
**Expected:** `Err` — `v0` is a SIMD register, not a GP operand
**Actual:** `Ok(Word(...))` — bit-identical to `umull x0, w0, w0`
**Minimal failing input:** `encode_umull(&[Operand::Reg("x0".into()), Operand::Reg("w0".into()), Operand::Reg("v0".into())])`

Differential check: `echo 'umull x0, w0, v0' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a SIMD-named operand is read as the same-numbered GP
register with no diagnostic; a SIMD destination clobbers an unrelated GP register.

## Suggested Fix

Apply a shared `reject_fp_simd` check across all three operand positions before
encoding.

## Regression Property

Failing witness: `umull_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_mul_madd_msub_umaddl_umull_pbt::umull_rejects_fp_simd_in_any_position -- --ignored
```


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/307
