# Bug Report: `encode_mul` silently accepts FP/SIMD registers in any operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Severity:** High

## Summary

`encode_mul` resolves operands through `get_reg`/`parse_reg_num`, which accept
FP/SIMD register-name prefixes (`d`/`s`/`q`/`v`/`h`/`b`) and return the trailing
lane number as the register field. MUL operates only on **general-purpose**
registers; a SIMD name in `Rd`/`Rn`/`Rm` is a register-class violation the
reference assembler rejects. The encoder accepts it and emits an instruction
bit-identical to the same-numbered GP register.

For example `mul x0, x0, v2` encodes identically to `mul x0, x0, x2` — i.e. it
reads GP register `x2`, not the SIMD register `v2` the source text named.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let prefix = name.chars().next()?;
    match prefix {
        'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // FP/SIMD lumped with GP
            let num: u32 = name[1..].parse().ok()?;
            ...
```

No register-class check distinguishes GP (`x`/`w`) from FP/SIMD.

## Reproduction

**Input:** `mul x0, x0, v2`
**Expected:** `Err` — `v2` is a SIMD register, not a GP operand
**Actual:** `Ok(Word(...))` — bit-identical to `mul x0, x0, x2`
**Minimal failing input:** `encode_mul(&[Operand::Reg("x0".into()), Operand::Reg("x0".into()), Operand::Reg("v0".into())])`

Differential check: `echo 'mul x0, x0, v2' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: code that names a SIMD register for a GP-only multiply
assembles against a *different* register (the same-numbered GP register) with no
diagnostic, yielding wrong data and clobbering an unrelated GP register when the
SIMD name is in the destination.

## Suggested Fix

Validate the register class of each GP operand before encoding (only `x`/`w` plus
`xzr`/`wzr` are legal here), e.g. via a shared `reject_fp_simd` helper applied to
all operand positions.

## Regression Property

Failing witness: `mul_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_mul_madd_msub_umaddl_umull_pbt::mul_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)
