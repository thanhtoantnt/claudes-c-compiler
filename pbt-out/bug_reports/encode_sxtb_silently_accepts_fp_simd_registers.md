# Bug Report: `encode_sxtb` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtb`
**Severity:** Medium

## Summary

`encode_sxtb` (`SXTB <Rd>,<Rn>`, alias of `SBFM` with `imms=7`) resolves both
operands through the shared `get_reg`/`parse_reg_num` helpers, which accept the
FP/SIMD register prefixes `d`/`s`/`q`/`v`/`h`/`b` and map them to the
same-numbered general-purpose register. `is_64bit_reg` returns `false` for all
of these prefixes, so the result is silently encoded as a **32-bit** `SBFM`
with the FP lane number placed into the `Rd`/`Rn` fields. SXTB is a scalar
general-purpose instruction; FP/SIMD operands are **unallocated**.

Consequently `sxtb d0, w0` is **silently** encoded as `sxtb w0, w0`, returning
`Ok(Word(...))` with no diagnostic.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    match prefix {
        'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // <- FP/SIMD accepted
            let num: u32 = name[1..].parse().ok()?;
            if num <= 31 { Some(num) } else { None }
        }
        ...
```

`encode_sxtb` reads `is_64` from `is_64bit_reg` (false for `d/s/q/v/h/b`) and
never validates that the operand is a general-purpose register.

## Reproduction

**Input:** `sxtb d0, w0`
**Expected:** `Err` — FP/SIMD registers are not valid SXTB operands
**Actual:** `Ok(Word(318774272))` (= `0x13001C00`, a 32-bit SBFM with `Rd=0, Rn=0`, indistinguishable from `sxtb w0, w0`)
**Minimal failing input (PBT-shrunk):** `encode_sxtb(&[Operand::Reg("d0".into()), wreg(0)])` (destination FP/SIMD, position 0)

Differential oracle:
```text
$ echo 'sxtb d0, d1' | clang --target=aarch64-linux-gnu -c -x assembler -
<stdin>:1:6: error: invalid operand for instruction
```

## Impact

Silent mis-compilation: an FP/SIMD register supplied to `SXTB` (in either
operand position) is assembled as if it were the same-numbered general-purpose
register, with no assembler error.

## Suggested Fix

Validate the register bank in `encode_sxtb` (reject `d`/`s`/`q`/`v`/`h`/`b`
prefixes in both `Rd` and `Rn`) before encoding.

## Regression Property

Failing witness: `wit_sxtb_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_sxtb_sxth_sxtw_fpsimd_pbt::wit_sxtb_rejects_fp_simd_in_any_position -- --ignored
```


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/343
