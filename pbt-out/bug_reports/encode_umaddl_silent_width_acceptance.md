# Bug Report: `encode_umaddl` performs no register-width validation

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_umaddl` (≈ line 665)

## Summary

`encode_umaddl` is the encoder for `UMADDL Xd, Wn, Wm, Xa` — an unsigned
multiply-add **long** instruction (`Xa + Wn * Wm`, widened to 64 bits). Per the
ARMv8 ARM it is defined **only** in the 64-bit (`sf=1`) form, and its operand-width
contract is fixed in both directions: the destination `Xd` and accumulator `Xa` MUST
be 64-bit (`X`) registers, while the two multiplier sources `Wn`/`Wm` MUST be 32-bit
(`W`) registers. (GNU `as` rejects any other combination.)

The function calls `get_reg` for all four operands but **discards the returned
`is_64` flag on every one** (binding it to `_`). It therefore performs **zero**
width checks: any mixture of `W`/`X` operands is silently re-typed and emitted as a
word indistinguishable from a correctly-typed `UMADDL`. The assembled instruction is
architecturally UNDEF / unpredictable.

This is the UMADDL sibling of the already-filed `encode_smaddl_silent_width_acceptance`
finding, and the same class of defect as `encode_umull_mixed_width_operands`.

## Reproduction

```text
test ...umaddl_rejects_w_destination_register ... FAILED   (umaddl w0, w1, w2, x3)
test ...umaddl_rejects_x_source_registers      ... FAILED   (umaddl x0, x1, x2, x3)
minimal failing input (both): rd = 0, rn = 0, rm = 0, ra = 0
```

Both malformed inputs encode to `Ok(Word(0x9BA00000))` — exactly the word a correctly
typed `umaddl x0, w1, w2, x3` would produce — with no diagnostic. Canonical case:

```text
umaddl w0, w1, w2, x3   →   Ok(Word(0x9BA00000))   (expected Err)
```

(An all-`X` source form, `umaddl x0, x1, x2, x3`, encodes to the same `0x9BA00000`.)

## Root cause

```rust
pub(crate) fn encode_umaddl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded — Rd must be X (64-bit)
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded — Rn must be W (32-bit)
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded — Rm must be W (32-bit)
    let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded — Ra must be X (64-bit)
    // sf is hardwired to 1 below regardless of the operands' actual widths
    let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` (encoder/mod.rs:956) faithfully returns the parsed width as its second
tuple element; it is the encoder that throws it away.

## Impact

A malformed source line is assembled without any diagnostic into an instruction the
architecture does not define. Because a wrong-width operand lands in the same bit
field as its valid counterpart, the corruption is invisible at the encoding level —
it can only surface as silent mis-execution on target, or as a divergence from a
reference assembler (GNU `as` rejects both forms above).

## Suggested fix

Enforce the `Xd, Wn, Wm, Xa` width contract (64-bit dest + accumulator; 32-bit
sources) before emitting:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
if !rd64 {
    return Err("umaddl destination must be a 64-bit (X) register".to_string());
}
if !ra64 {
    return Err("umaddl accumulator must be a 64-bit (X) register".to_string());
}
if rn64 || rm64 {
    return Err("umaddl source operands must be 32-bit (W) registers".to_string());
}
let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
    | (ra << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```
