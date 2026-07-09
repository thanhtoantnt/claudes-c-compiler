# BUG: `encode_csneg` silently accepts mixed-width (X/W) register operands

**File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
**Function:** `encode_csneg`
**Status:** Confirmed failing property
`prop_encode_csneg_tests::prop_rejects_mixed_width_operands`

## Summary

`encode_csneg` derives the `sf` (operand-size) bit **only** from the destination
register `Rd` and silently ignores the widths of `Rn` and `Rm`. Consequently it
accepts architecturally **UNPREDICTABLE/UNDEFINED** instructions that mix a
64-bit (`xN`) register with a 32-bit (`wN`) register, e.g. `csneg w0, w0, x0, eq`
or `csneg x0, w1, w2, eq`, emitting a malformed word instead of returning `Err`.

## Root cause

```rust
pub(crate) fn encode_csneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width taken ONLY from Rd
    let (rn, _) = get_reg(operands, 1)?;        // width DISCARDED
    let (rm, _) = get_reg(operands, 2)?;        // width DISCARDED
    ...
    let sf = sf_bit(is_64);                      // sf == Rd's width only
    let word = ((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The widths returned for `Rn` and `Rm` are bound to `_` and never compared to
`is_64`, so a mismatch never produces an error.

## Architectural basis

Per the ARM Architecture Reference Manual, **Conditional Select (Negate)**
(`CSNEG`): the single `sf` field at bit `[31]` applies **uniformly** to `Rd`,
`Rn`, and `Rm` — all three register operands must be the same width (all `W` for
`sf=0`, all `X` for `sf=1`). Mixing widths is **UNPREDICTABLE**. Real assemblers
reject it:

- **GAS:** operand-size / register-bank mismatch error
- **LLVM-MC:** `error: invalid operand for instruction` / operand-size mismatch

No cited spec permits mixed-width conditional-select encodings. (Note: no
`llvm-mc` or aarch64 cross-assembler was available in this environment for live
differential validation; the local `as` is x86 binutils. The claim rests on the
ARM ARM encoding definition.)

## Reproduction

```bash
cargo test --lib prop_encode_csneg_tests::prop_rejects_mixed_width_operands
```

Minimal failing input (from proptest shrink):

```
csneg w0, w0, x0, eq   ->   encode_csneg returns Ok(Word(0x5A800000))
                           (sf=0 32-bit instruction, but Rm = x0 is 64-bit)
```

Other failing shapes (all currently accepted):

- `csneg x0, w0, w0, eq`  (sf=1, but Rn/Rm are 32-bit)
- `csneg x0, x0, w0, eq`  (sf=1, Rm 32-bit)
- `csneg w0, x0, w0, eq`  (sf=0, Rn 64-bit)
- any permutation where Rd/Rn/Rm widths are not all equal

## Suggested fix

In `encode_csneg`, capture the widths of `Rn` and `Rm` and require all three to
agree:

```rust
let (rd, rd_is64) = get_reg(operands, 0)?;
let (rn, rn_is64) = get_reg(operands, 1)?;
let (rm, rm_is64) = get_reg(operands, 2)?;
if rd_is64 != rn_is64 || rd_is64 != rm_is64 {
    return Err("csneg: Rd, Rn, Rm must all be the same width".to_string());
}
let sf = sf_bit(rd_is64);
```

The same latent gap affects the sibling conditional-select encoders
`encode_csel`, `encode_csinc`, `encode_csinv` (they all bind `Rn`/`Rm` widths to
`_`), so the fix should be applied consistently across the family.

## Related finding (pre-existing, not introduced here)

`prop_encode_csneg_tests::prop_rejects_fp_simd_registers` also fails:
`encode_csneg` accepts FP/SIMD register names (`d/s/q/v/h/b`) as if they were GP
registers, because `parse_reg_num` accepts every such prefix and `get_reg` derives
`sf` only from `is_64bit_reg`. This is the same validation-gap theme and is noted
separately in that property.
