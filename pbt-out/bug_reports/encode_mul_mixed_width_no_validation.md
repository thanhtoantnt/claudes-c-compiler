# BUG: `encode_mul` silently accepts mixed-width source registers

**Function:** `src/backend/arm/assembler/encoder/data_processing.rs::encode_mul`
**Severity:** High (silent mis-encoding of an ISA-invalid instruction)
**File:** `src/backend/arm/assembler/encoder/data_processing.rs`

## Summary

`encode_mul` derives the `sf` (operand-size) bit **only** from the destination
register and performs **no** check that the two source registers (`Rn`, `Rm`)
share the destination's width. The ARMv8 ARM `MUL` alias requires all three
operands (`<Rd>`, `<Rn>`, `<Rm>`) to be the same width; a mixed-width form such
as `mul x0, w1, w2` has **no valid encoding** and must be rejected. Instead,
`encode_mul` happily emits a 64-bit `MADD` with the `W`-register *numbers*
plugged into the `Rn`/`Rm` fields, producing an instruction that does not exist
in the architecture and is rejected by the system assembler (`llvm-mc`).

## Root cause

```rust
pub(crate) fn encode_mul(operands: &[Operand]) -> Result<EncodeResult, String> {
    // NEON vector form: MUL Vd.T, Vn.T, Vm.T
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_mul(operands);
    }
    // MUL Rd, Rn, Rm is MADD Rd, Rn, Rm, XZR
    let (rd, is_64) = get_reg(operands, 0)?;   // <-- width taken from Rd ONLY
    let (rn, _) = get_reg(operands, 1)?;        // <-- width discarded (`_`)
    let (rm, _) = get_reg(operands, 2)?;        // <-- width discarded (`_`)
    let sf = sf_bit(is_64);
    let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(reg_num, is_64)` for every operand, but the `is_64` of `Rn`
and `Rm` is thrown away via `(rn, _)` / `(rm, _)`. There is no assertion that
they equal `is_64` of `Rd`. Compare with the sibling `encode_smull`, which — per
its own negative-contract properties — *does* reject a wrong-width destination.
`encode_mul` has no such guard on either source.

## Reproduction

### Failing property (this campaign)

```text
test ...::tests::mul_rejects_mixed_register_widths ... FAILED

thread '...::mul_rejects_mixed_register_widths' panicked:
assertion failed: encode_mul(&ops1).is_err()
minimal failing input: n = 0
```

`encode_mul(&[xreg(0), wreg(0), wreg(0)])` returns `Ok(Word(0x9B007C00))`
instead of `Err`.

### Differential validation against `llvm-mc-18` (LLVM, the reference assembler)

```
$ printf 'mul x0, w1, w2\nmul w0, x1, x2\nmul x0, x1, x2\nmul w0, w1, w2\n' \
  | llvm-mc-18 --triple=aarch64 --show-encoding
<stdin>:1:9: error: invalid operand for instruction
mul x0, w1, w2          # <-- encode_mul accepts; llvm-mc REJECTS
        ^
<stdin>:2:9: error: invalid operand for instruction
mul w0, x1, x2          # <-- encode_mul accepts; llvm-mc REJECTS
        ^
mul x0, x1, x2          // encoding: [0x20,0x7c,0x02,0x9b]  = 0x9B027C20
mul w0, w1, w2          // encoding: [0x20,0x7c,0x02,0x1b]  = 0x1B027C20
```

`llvm-mc` rejects exactly the two mixed-width cases that `encode_mul` silently
encodes. The two valid same-width encodings **match this campaign's reference
oracle** (`0x9B007C00 | (rm<<16) | (rn<<5) | rd`) byte-for-byte, which both
confirms the reference and pinpoints the validation gap.

## Impact

- An assembly source containing `mul x0, w1, w2` (a likely authoring typo or a
  generated-code mistake) is assembled to a word that the system loader/linker
  or a later `llvm-mc`-based verification step will reject — but only **after**
  this compiler has already "succeeded", deferring the failure and obscuring its
  origin.
- The emitted word reuses the `W` register *number* as if it were an `X` source,
  so the resulting instruction is not merely a wrong-width multiply — it has no
  defined semantics in the ARMv8 ISA.
- Hardens a class of "silent truncation / silent mis-encoding" bugs: there is no
  range/width validation on the two source operands at all.

## Spec reference

ARM Architecture Reference Manual for A-profile, `MUL` (alias of `MADD`):

> `MUL <Rd>, <Rn>, <Rm>`
> Alias of: `MADD <Rd>, <Rn>, <Rm>, <ZR>`
> Operands `<Rd>`, `<Rn>`, `<Rm>` are all `<Wd>` or all `<Xd>` (same width).

The shared-width constraint is an architectural precondition, not an assembler
convenience; violating it is `INVALID` per `llvm-mc`.

## Suggested fix

Validate that all three operands share the same width before emitting:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;
if rn_64 != is_64 || rm_64 != is_64 {
    return Err("mul operands must all be the same register width".to_string());
}
let sf = sf_bit(is_64);
let word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

## Coverage

Properties added in `src/backend/arm/assembler/encoder/data_processing.rs`
(`mod tests`, `proptest!` block immediately after the existing `smull` block):

| # | Property | Status | Role |
|---|----------|--------|------|
| P1 | `mul_reference_encoding` | PASS | Reference oracle (spec-derived constants, 0..=31, both widths); cross-checked against `llvm-mc`. |
| P2 | `mul_field_placement` | PASS | Per-field placement of every fixed opcode bit-group and `Rm`/`Rn`/`Rd`, both widths. |
| P3 | `mul_sf_tracks_destination_width_only` | PASS | **Characterization** of the buggy behavior: `sf` follows `Rd` only, regardless of source widths. |
| P4 | `mul_rejects_mixed_register_widths` | **FAIL** | **Negative contract (spec):** mixed-width forms must return `Err`. This is the documenting failing test. |

P3 documents the current (incorrect) behavior; P4 asserts the spec-correct
behavior and is the property that fails, proving the gap.
