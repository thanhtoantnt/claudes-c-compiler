# Bug Report: `encode_adc` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_adc`
**Severity:** High

## Summary

`encode_adc` resolves operands through `get_reg`/`parse_reg_num`, which accepts FP/SIMD register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`) and returns the trailing lane number as the register field. ADC operates only on **general-purpose** registers; a SIMD name in `Rd`/`Rn`/`Rm` is a register-class violation the reference assembler rejects. The encoder accepts it and emits an instruction bit-identical to the same-numbered GP register, silently producing a wrong-operand instruction.

For example `adc x0, x1, v2` encodes identically to `adc x0, x1, x2` — i.e. it reads GP register `x2`, not the SIMD register `v2` the source text named.

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

**Input:** `adc x0, x1, v2`

**Expected:** `Err` — `v2` is a SIMD register, not a GP operand

**Actual:** `Ok(Word(...))` — bit-identical to `adc x0, x1, x2`

**Minimal failing input:** `encode_adc(&[Operand::Reg("x0".into()), Operand::Reg("x1".into()), Operand::Reg("v2".into())], false)`

Differential check: `echo 'adc x0, x1, v2' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: code that names a SIMD register for a GP-only instruction is assembled against a *different* register (the same-numbered GP register) with no diagnostic, yielding wrong data and, for the destination, clobbering an unrelated GP register.

## Suggested Fix

Validate the register class of each GP operand before encoding (only `x`/`w` plus `xzr`/`wzr` are legal here), for example:

```rust
fn reject_fp_simd(operands: &[Operand]) -> Result<(), String> {
    for op in operands {
        if let Operand::Reg(r) = op {
            if is_fp_reg(r) { return Err("ADC: FP/SIMD register not allowed".into()); }
        }
    }
    Ok(())
}
```

## Regression Property

Failing property: `adc_rejects_fp_simd_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::adc_rejects_fp_simd_in_any_position -- --ignored
prop_assert!(encode_adc(&[Operand::Reg("x0".into()), Operand::Reg("x1".into()), Operand::Reg("v2".into())], false).is_err());
```

**GitHub Issue:** (none)
