# Bug Report: `encode_smaddl` silently accepts a 32-bit destination register

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_smaddl`

## Summary

`encode_smaddl` is the encoder for `SMADDL Xd, Wn, Wm, Xa` — a signed multiply-add
long instruction that is defined **only** in the 64-bit (`sf=1`) form. There is no
32-bit variant. The function, however, calls `get_reg` and discards the returned
`is_64` flag for every operand, so it silently accepts a 32-bit `W` register as the
destination (and as the accumulator `Ra`) and emits a word indistinguishable from the
valid `X`-form encoding.

The decoded instruction is then architecturally UNDEF / unpredictable (an assembler
should reject `smaddl w0, w1, w2, x3`).

## Reproduction

Property `smaddl_rejects_w_destination_register` fails with the minimal input:

```text
smaddl w0, x1, x2, x3
```

Actual behavior: `encode_smaddl` returns `Ok(Word(0x9B200000))` (the same word it
would emit for `smaddl x0, x1, x2, x3`), i.e. `Ok` with no diagnostic.

```text
test backend::arm::assembler::encoder::data_processing::tests::smaddl_rejects_w_destination_register ... FAILED
minimal failing input: rd = 0, rn = 0, rm = 0, ra = 0
```

## Root cause

```rust
pub(crate) fn encode_smaddl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded
    // sf is hardwired to 1 below regardless of the operands' actual width
    let word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Impact

A malformed source line is assembled without warning into an instruction the
architecture does not define. Same class of defect as the existing
`encode_div_mixed_width_operands` and `encode_*_rejects_32bit_w_registers` findings
(SMULH/UMULH) already documented for this module.

## Suggested fix

Enforce the `Xd, Wn, Wm, Xa` width contract (and reject the destination/accumulator
being `W`, and the sources being `X`):

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
// SMADDL is 64-bit only: destination and accumulator must be X (64-bit),
// sources must be W (32-bit).
if !rd64 {
    return Err("smaddl destination must be a 64-bit (X) register".to_string());
}
if !ra64 {
    return Err("smaddl accumulator must be a 64-bit (X) register".to_string());
}
if rn64 || rm64 {
    return Err("smaddl source operands must be 32-bit (W) registers".to_string());
}
```

Note: the same defect exists in the sibling `encode_umaddl` (UMADDL Xd, Wn, Wm, Xa).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/96
