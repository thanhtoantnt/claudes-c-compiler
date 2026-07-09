# Bug Report: `encode_sbc` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sbc`
**Severity:** High

## Summary

`encode_sbc` resolves operands through `get_reg`/`parse_reg_num`, which accepts FP/SIMD register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`) and returns the trailing lane number as the register field. SBC operates only on **general-purpose** registers; a SIMD name in `Rd`/`Rn`/`Rm` is a register-class violation the reference assembler rejects. The encoder accepts it and emits an instruction bit-identical to the same-numbered GP register.

For example `sbc x0, x1, v2` encodes identically to `sbc x0, x1, x2` — reading GP register `x2` instead of the SIMD register `v2`.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let prefix = name.chars().next()?;
    match prefix {
        'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // FP/SIMD prefixes lumped with GP
            let num: u32 = name[1..].parse().ok()?;
            ...
```

No register-class check distinguishes GP (`x`/`w`) from FP/SIMD (`d`/`s`/`q`/`v`/`h`/`b`).

## Reproduction

**Input:** `sbc x0, x1, v2`

**Expected:** `Err` — `v2` is a SIMD register, not a GP operand

**Actual:** `Ok(Word(...))` — bit-identical to `sbc x0, x1, x2`

**Minimal failing input:** `encode_sbc(&[Operand::Reg("x0".into()), Operand::Reg("x1".into()), Operand::Reg("v2".into())], false)`

Differential check: `echo 'sbc x0, x1, v2' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a SIMD register named for a GP-only instruction is silently replaced by the same-numbered GP register, producing wrong data and potentially clobbering an unrelated GP destination, with no assembler error.

## Suggested Fix

Validate the register class of each GP operand before encoding (only `x`/`w` plus `xzr`/`wzr` are legal), using a `reject_fp_simd` helper.

## Regression Property

Failing property: `sbc_rejects_fp_simd_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::sbc_rejects_fp_simd_in_any_position -- --ignored
prop_assert!(encode_sbc(&[Operand::Reg("x0".into()), Operand::Reg("x1".into()), Operand::Reg("v2".into())], false).is_err());
```


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/299
