# Bug: `encode_smaddl` does not validate register widths (silent acceptance of invalid encodings)

## Location
`src/backend/arm/assembler/encoder/data_processing.rs` — `encode_smaddl`

## Summary
`encode_smaddl` reads only the **number** of each register and discards the
**width** flag returned by `get_reg`. It therefore silently accepts operand
combinations that have no valid SMADDL encoding per the ARMv8 ARM, producing a
word as if the registers were the correct width.

## Evidence (source)
```rust
pub(crate) fn encode_smaddl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded
    // SMADDL: 1 00 11011 001 Rm 0 Ra Rn Rd
    let word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)`, but every `is_64` is bound to `_`.

## Spec contract (ARMv8 ARM, §C4.1.65 / SMADDL)
```
SMADDL <Xd>, <Wn>, <Wm>, <Xa>
```
- `Xd` MUST be a 64-bit (X) register.
- `Wn`, `Wm` MUST be 32-bit (W) registers.
- `Xa` MUST be a 64-bit (X) register.

A mismatched width (e.g. a W destination, an X source, or a W accumulator) is
**unallocated** and an assembler must reject it.

## Actual behaviour
The encoder returns `Ok(Word(...))` and reuses the raw 5-bit register number,
so e.g. all of the following (every one invalid) are accepted and encoded:

| Mnemonic operands             | Accepted? |
|-------------------------------|-----------|
| `smaddl w0, w1, w2, w3`       | ✅ (wrong: dest + accumulator) |
| `smaddl x0, x1, x2, x3`       | ✅ (wrong: sources are 64-bit) |
| `smaddl w0, x1, x2, w3`       | ✅ (wrong: all four)           |
| `smaddl x0, w1, w2, w3`       | ✅ (wrong: accumulator)        |

Only the numerically-correct form (`x_, w_, w_, x_`) should be accepted.

## Impact
- **Silent mis-assembly.** A user typo (`w` instead of `x`, or vice-versa) is
  assembled without diagnostic, yielding an instruction with a different
  semantic width than intended. Because the 5-bit number happens to fit,
  nothing downstream catches it.
- Inconsistent with the rest of the toolchain, where `get_reg`'s `is_64` is
  the standard width signal (used for the `sf` bit in ADD/SUB/LOGICAL/etc.).
  Here `sf` is correctly *hardcoded* to 1, but the *operand* widths are still
  required by the alias and go unchecked.

## Suggested fix
Validate widths against the SMADDL alias before encoding, e.g.:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
if !rd64 { return Err("smaddl destination must be a 64-bit (X) register".into()); }
if  rn64 { return Err("smaddl source Wn must be a 32-bit (W) register".into()); }
if  rm64 { return Err("smaddl source Wm must be a 32-bit (W) register".into()); }
if !ra64 { return Err("smaddl accumulator Xa must be a 64-bit (X) register".into()); }
```

The same gap affects the sibling long-form encoders `encode_smull`, `encode_umull`,
and `encode_umaddl` (all bind `is_64` to `_`).

## Test coverage
The new `smaddl_props` module (`src/backend/arm/assembler/encoder/data_processing.rs`)
contains 5 passing properties (reference encoding, field placement, sf-invariance,
SMADDL≡SMULL@Ra=XZR differential, and missing/non-register rejection).

A regression property asserting that wrong-width operands are rejected was
**deliberately not added as a passing test** because it would fail against
current behaviour; it is captured here instead. Once the fix lands, add:

```rust
#[test]
fn smaddl_rejects_wrong_width_operands(n in 0u32..=30) {
    let ops = vec![wreg(n), wreg(n), wreg(n), wreg(n)]; // smaddl w0,w1,w2,w3
    prop_assert!(encode_smaddl(&ops).is_err());
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/95
