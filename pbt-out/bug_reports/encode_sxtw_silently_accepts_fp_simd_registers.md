# Bug Report: `encode_sxtw` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtw`
**Severity:** Medium

## Summary

`encode_sxtw` (`SXTW <Xd>,<Wn>`, alias of `SBFM` with `imms=31`, 64-bit only)
resolves both operands through the shared `get_reg`/`parse_reg_num` helpers,
which accept the FP/SIMD register prefixes `d`/`s`/`q`/`v`/`h`/`b` and map them
to the same-numbered general-purpose register. `encode_sxtw` hardcodes `sf=1`
and `N=1` and ignores the `is_64` flag of both operands, so the result is
silently encoded as a **64-bit** `SBFM` with the FP lane number placed into the
`Rd`/`Rn` fields. SXTW is a scalar general-purpose instruction; FP/SIMD
operands are **unallocated**.

Consequently `sxtw d0, x0` is **silently** encoded as `sxtw x0, w0`, returning
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

`encode_sxtw` discards `get_reg`'s `is_64` flag for both operands and never
validates that the operand is a general-purpose register.

## Reproduction

**Input:** `sxtw d0, x0`
**Expected:** `Err` — FP/SIMD registers are not valid SXTW operands
**Actual:** `Ok(Word(2470476800))` (= `0x93407C00`, a 64-bit SBFM with `Rd=0, Rn=0`, indistinguishable from `sxtw x0, w0`)
**Minimal failing input (PBT-shrunk):** `encode_sxtw(&[Operand::Reg("d0".into()), xreg(0)])` (destination FP/SIMD, position 0)

Differential oracle:
```text
$ echo 'sxtw v0, v1' | clang --target=aarch64-linux-gnu -c -x assembler -
<stdin>:1:6: error: invalid operand for instruction
```

## Impact

Silent mis-compilation: an FP/SIMD register supplied to `SXTW` (in either
operand position) is assembled as if it were the same-numbered general-purpose
register, with no assembler error.

## Suggested Fix

Validate the register bank in `encode_sxtw` (reject `d`/`s`/`q`/`v`/`h`/`b`
prefixes in both `Rd` and `Rn`) before encoding.

## Regression Property

Failing witness: `wit_sxtw_rejects_fp_simd_in_any_position`

```text
cargo test --lib data_processing_sxtb_sxth_sxtw_fpsimd_pbt::wit_sxtw_rejects_fp_simd_in_any_position -- --ignored
```

**GitHub Issue:** (none)
