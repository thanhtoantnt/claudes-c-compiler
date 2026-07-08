# Bug Report: `encode_div` performs no register-width validation

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_div` (≈ line 618)

## Summary

`encode_div` is the shared encoder for `SDIV` (signed) and `UDIV` (unsigned)
divide. Per the ARMv8 ARM, the "Data-processing (2 source)" form requires
`<Rd>, <Rn>, <Rm>` to **all share the same register width** — all `W` (32-bit)
or all `X` (64-bit). The `sf` bit (bit 31) then selects the width for the whole
instruction. (GNU `as` / `llvm-mc` reject any mismatched combination.)

The function calls `get_reg` for all three operands but **discards the returned
`is_64` flag on `Rn` and `Rm`** (both bound to `_`), deriving `sf` **only** from
the destination register. It therefore performs **zero** width checks: a
mixed-width form such as `sdiv x0, w1, w2` is silently re-typed as a 64-bit
instruction (sf=1) with `W`-numbered sources, and assembled with no diagnostic.
The emitted word is indistinguishable from a correctly typed `sdiv x0, x1, x2`
except that the assembler has silently corrupted the source operand semantics.

This is the SDIV/UDIV sibling of the already-filed `encode_mul` /
`encode_smaddl` / `encode_umaddl` silent-width findings.

## Reproduction

```text
test ...div_rejects_mixed_register_widths ... FAILED
minimal failing input: n = 0, unsigned = false
```

Both malformed inputs succeed instead of erroring:

```text
sdiv x0, w1, w2   →   Ok(Word(0x9AC00C20))   (expected Err)
sdiv w0, x1, x2   →   Ok(Word(0x1AC00C20))   (expected Err)
udiv x0, w1, w2   →   Ok(Word(0x9AC00820))   (expected Err)
```

(The `sf` bit tracks only the destination: `x` dest → sf=1, `w` dest → sf=0,
regardless of the source register widths.)

## Root cause

```rust
pub(crate) fn encode_div(operands: &[Operand], unsigned: bool) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width kept → drives sf
    let (rn, _) = get_reg(operands, 1)?;       // is_64 DISCARDED — Rn width never checked
    let (rm, _) = get_reg(operands, 2)?;       // is_64 DISCARDED — Rm width never checked
    let sf = sf_bit(is_64);
    let o1 = if unsigned { 0u32 } else { 1u32 };
    // Data-processing (2 source): sf 0 S=0 11010110 Rm 00001 o1 Rn Rd
    let word = (sf << 31) | (0b0011010110 << 21) | (rm << 16)
        | (0b00001 << 11) | (o1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` (encoder/mod.rs:956) faithfully returns the parsed width as its second
tuple element; it is the encoder that throws it away for the two sources.

## Impact

A malformed source line is assembled without any diagnostic into an instruction
whose operand widths do not match its `sf` bit. Because a wrong-width operand
lands in the same 5-bit register field as its valid counterpart, the corruption
is invisible at the encoding level — it can only surface as silent
mis-execution on target, or as a divergence from a reference assembler
(GNU `as` / `llvm-mc` reject these forms).

## Suggested fix

Validate that all three operands share one width, matching the destination:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;
if rn_64 != is_64 || rm_64 != is_64 {
    return Err(format!(
        "sdiv/udiv operands must all share the destination's register width ({}-bit)",
        if is_64 { 64 } else { 32 }
    ));
}
let sf = sf_bit(is_64);
// ... unchanged emission ...
```
