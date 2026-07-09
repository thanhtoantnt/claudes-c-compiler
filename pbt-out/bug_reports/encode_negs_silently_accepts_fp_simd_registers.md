# Bug Report: `encode_negs` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

`encode_negs` (`NEGS <Rd>,<Rm>`) resolves operands through `get_reg`/`parse_reg_num`, which accepts FP/SIMD register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`) and returns the trailing lane number as the register field. NEGS operates only on **general-purpose** registers; a SIMD name in `Rd`/`Rm` is a register-class violation the reference assembler rejects. The encoder accepts it and emits an instruction bit-identical to the same-numbered GP register.

For example `negs x0, v1` encodes identically to `negs x0, x1` — reading GP register `x1` instead of the SIMD register `v1`.

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

**Input:** `negs x0, v1`

**Expected:** `Err` — `v1` is a SIMD register, not a GP operand

**Actual:** `Ok(Word(...))` — bit-identical to `negs x0, x1`

**Minimal failing input:** `encode_negs(&[Operand::Reg("x0".into()), Operand::Reg("v1".into())])`

Differential check: `echo 'negs x0, v1' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a SIMD register named for a GP-only instruction is silently replaced by the same-numbered GP register, producing wrong data and condition flags, with no assembler error.

## Suggested Fix

Validate the register class of each GP operand before encoding (only `x`/`w` plus `xzr`/`wzr` are legal), using a `reject_fp_simd` helper.

## Regression Property

Failing property: `negs_rejects_fp_simd_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::negs_rejects_fp_simd_in_any_position -- --ignored
prop_assert!(encode_negs(&[Operand::Reg("x0".into()), Operand::Reg("v1".into())]).is_err());
```


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/289
