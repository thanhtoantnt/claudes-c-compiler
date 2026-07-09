# BUG REPORT — `encode_csinc` silently accepts mixed register widths

**File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
**Function:** `encode_csinc`
**Focus:** mixed-width validation
**Severity:** correctness / silent mis-assembly (would assemble invalid AArch64)

## Summary

`encode_csinc` derives the `sf` (width) bit from **only** the destination register `Rd`
(operand 0) and **discards** the widths of `Rn` and `Rm`. As a result it silently accepts
mixed-width operand combinations such as `csinc w0, x1, x2, eq`, coercing them to `Rd`'s
width instead of rejecting them. The ARM Architecture Reference Manual defines CSINC with a
single `sf` field that applies to the **whole** instruction, so `Rd`, `Rn`, and `Rm` must all
be the same width. GNU `as` and LLVM-MC reject mixed-width forms with an
`operand size mismatch` error.

## Root cause

```rust
pub(crate) fn encode_csinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width taken from Rd ONLY
    let (rn, _)    = get_reg(operands, 1)?;     // Rn width DISCARDED
    let (rm, _)    = get_reg(operands, 2)?;     // Rm width DISCARDED
    ...
    let sf = sf_bit(is_64);                      // = Rd's width
    let word = (sf << 31) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)` but the `is_64` for `Rn`/`Rm` is bound to `_` and never
compared against `Rd`'s width. There is no consistency check and no error path for a mismatch.

## Evidence (from the new property tests)

Added to `mod prop_encode_csinc_tests` in the same file:

| Property | Input | Expected | Actual |
|---|---|---|---|
| `prop_rejects_mixed_width_registers` (FAILS) | `csinc w0, w0, x0, eq` | `Err` | `Ok(Word(0x1a800400))` |
| `prop_rejects_zr_width_mismatch` (FAILS) | `csinc xzr, w0, x2, eq` | `Err` | `Ok(Word(...))` (accepted) |
| `prop_uniform_widths_always_succeed` (passes) | `csinc w0, w0, w0, eq` | `Ok`, sf=0 | `Ok(Word(0x1a800400))` |

Note that `csinc w0, w0, x0, eq` and `csinc w0, w0, w0, eq` encode to the **same** 32-bit word
`0x1a800400` — the 64-bit `Rm = x0` is silently reinterpreted as a 32-bit operand. This is a
silent mis-assembly: the user wrote a 64-bit source register and got a 32-bit instruction with
no diagnostic.

The zero-register aliases (`xzr` 64-bit / `wzr` 32-bit) carry the same width and exhibit the
identical bug (`prop_rejects_zr_width_mismatch`).

## Architectural basis

ARM ARM, *Conditional Select (increment)* — CSINC encoding:
```
sf  0 0  11010100  Rm  cond  0 1  Rn  Rd
```
`sf` is a single instruction-wide field. The assembler-level operands `Rd`, `Rn`, `Rm` are
either all `X`-registers (`sf=1`) or all `W`-registers (`sf=0`); a mixture is not a valid
instruction encoding.

## Suggested fix

Validate that all three register operands share one width before encoding, e.g.:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if rd_is_64 != rn_is_64 || rn_is_64 != rm_is_64 {
    return Err("csinc: all registers must have the same width (X or W)".to_string());
}
```

The same width-discarding pattern (`let (rn, _) = ...; let (rm, _) = ...;`) is present in the
sibling encoders `encode_csel`, `encode_csinv`, `encode_csneg` and the `cinc/cinv/cneg`
aliases, so they very likely share this defect.

## Related observation (out of scope, pre-existing)

`prop_rejects_invalid_operands` case 10 also fails: `csinc d0, x0, x0, eq` (an FP/SIMD
register as `Rd`) is accepted because `parse_reg_num("d0") == Some(0)`. CSINC is defined only
on GP (X/W) registers. This is an independent validation gap (FP/SIMD register acceptance),
not the mixed-width defect above.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/168
