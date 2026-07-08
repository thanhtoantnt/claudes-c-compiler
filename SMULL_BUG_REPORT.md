# Bug Report: `encode_smull` does not validate operand register widths

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_smull`

## Summary

`encode_smull` silently accepts operand register widths that are invalid for the
`SMULL` mnemonic and emits an encoding that does not match the source text, instead
of rejecting them with an error.

The ARMv8 ARM defines the signature as:

```
SMULL <Xd>, <Wn>, <Wm>     // alias of SMADDL <Xd>, <Wn>, <Wm>, <XZR>
```

- `<Xd>` MUST be a 64-bit (X) register.
- `<Wn>` and `<Wm>` MUST be 32-bit (W) registers.

## Root cause

```rust
pub(crate) fn encode_smull(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    // sf hardcoded to 1
    let word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16)
        | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)` but all three `is_64` flags are bound to `_` and
ignored. `sf` is hardcoded to `1`.

## Observed consequences

1. **Wrong-width destination accepted.** `smull w0, w1, w2` is accepted and encodes
   identically to `smull x0, w1, w2` — i.e. it emits `SMADDL X0, W1, W2, XZR`,
   silently writing a **64-bit** result to `x0`. The mnemonic `smull w0, ...` has no
   valid encoding; GNU `as` rejects it with `Error: operand size mismatch`.
2. **Wrong-width sources accepted.** `smull x0, x1, x2` is accepted and encoded with
   the source register numbers placed in the `Rn`/`Rm` fields even though `SMULL`
   requires `W` sources. Only the low 5 bits of the register name are used, so the
   intended width is lost; GNU `as` rejects this with an operand-size error.

In all cases the encoder returns `Ok` rather than `Err`.

## Evidence

Property `smull_sf_always_set_regardless_of_source_width` (added in this campaign)
passes for both `X` and `W` sources, proving the encoder ignores source width. The
hardcoded `(1u32 << 31)` proves destination width is also ignored. The fixed-bit
encoding itself (`0x9B207C00 | (rm<<16) | (rn<<5) | rd`) is otherwise correct, as
verified by `smull_reference_encoding` / `smull_field_placement` / `smull_matches_armv8_reference`.

## Suggested fix

Validate widths against the `SMULL` signature and return `Err` on mismatch, e.g.:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if !rd_is_64 {
    return Err("smull destination must be a 64-bit (X) register".into());
}
if rn_is_64 || rm_is_64 {
    return Err("smull sources must be 32-bit (W) registers".into());
}
```

The same pattern (`is_64` discarded, width unchecked) appears in `encode_umull`,
`encode_smaddl`, `encode_umaddl`, `encode_smulh`, `encode_umulh`, and
`encode_mulh`-style long/3-source encoders — worth auditing together.
