# Bug Report: `encode_msub` silently accepts mixed-width register operands

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_msub`

## Summary

`encode_msub` derives the instruction width (`sf`) **only** from the destination register `Rd` and discards the width class of the three source operands `Rn`, `Rm`, and `Ra` (they are bound to `_`). AArch64 MSUB requires all four operands to be the same width (all 32-bit W or all 64-bit X); mixed W/X operands are undefined and must be rejected with a diagnostic. This encoder accepts them silently and encodes the `sf` bit from `Rd` alone.

## Function under test

```rust
pub(crate) fn encode_msub(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // width discarded
    let (rm, _) = get_reg(operands, 2)?;   // width discarded
    let (ra, _) = get_reg(operands, 3)?;   // width discarded
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (1 << 15) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Reproduction

Characterization test run during the PBT campaign:

```text
msub x0, w1, x2, x3      // mixed widths: 64-bit dest + 32-bit source
```

Actual result:

```text
Ok(Word(0x9B028C20))
```

Field decode of `0x9B028C20`:

| field | bits | value | meaning |
|-------|------|-------|---------|
| sf    | 31   | 1     | 64-bit, taken from `x0` |
| op    | 30:21| 0xD8  | MSUB opcode (correct) |
| Rm    | 20:16| 2     | x2 |
| o0    | 15   | 1     | MSUB (not MADD) |
| Ra    | 14:10| 3     | x3 |
| Rn    | 9:5  | 1     | register 1 (programmer wrote **w1**) |
| Rd    | 4:0  | 0     | x0 |

The `sf` bit is 1 (64-bit) purely because `Rd` is `x0`. The programmer's 32-bit intent for `Rn` (`w1`) is silently dropped: the hardware will execute a 64-bit `MSUB X0, X1, X2, X3`, reading the full 64-bit `X1` instead of the intended 32-bit `W1`. No error is returned.

A reference assembler (e.g. GNU `as` for AArch64) rejects this:

```text
Error: operand mismatch -- `msub x0,w1,x2,x3'
```

## Impact

- **Silent mis-assembly of invalid source.** A mixed-width `msub` (a programming error) is assembled without diagnostic, producing an instruction whose execution width contradicts one or more operands as written.
- **Width is taken only from `Rd`.** Symmetric variants mislead: `msub w0, x1, x2, x3` assembles with `sf=0` and the hardware treats registers 1/2/3 as W aliases (upper bits ignored), again contradicting the programmer's X notation.
- The bug hides programmer errors that a correct assembler surfaces, so object code can diverge from intent with no warning.

## Suggested fix

Capture the width of all four operands and require them to match `Rd`'s width:

```rust
pub(crate) fn encode_msub(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, rn64)  = get_reg(operands, 1)?;
    let (rm, rm64)  = get_reg(operands, 2)?;
    let (ra, ra64)  = get_reg(operands, 3)?;
    if rn64 != is_64 || rm64 != is_64 || ra64 != is_64 {
        return Err("msub register operands must have matching widths".to_string());
    }
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (1 << 15) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Related

Same class of bug as `encode_madd_mixed_width_operands.md`, `encode_div_mixed_width_operands.md`,
`encode_adc_silent_mixed_width.md`, `encode_sbc_silent_mixed_width.md`, `encode_eon_mixed_register_widths.md`.
Filed separately per function as requested; each affected encoder needs its own validation.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/68
