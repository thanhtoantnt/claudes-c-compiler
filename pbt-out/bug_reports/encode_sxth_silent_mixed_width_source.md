# Bug Report: `encode_sxth` silently accepts architecturally invalid mixed-width source operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxth`
**Severity:** Medium (silent acceptance of an invalid instruction form; diverges from the reference assembler)

## Summary

`encode_sxth` derives the operation width *only* from the destination register (`get_reg(operands, 0)`) and ignores the width of the source register `Rn`. Per the ARMv8 ARM, `SXTH` is an alias of `SBFM`, whose encoding requires the `N` bit to equal `sf` — i.e. the source and destination registers must be the same width. The reference assembler (llvm-mc-18, triple `aarch64`) therefore rejects `sxth w0, x0` ("error: invalid operand for instruction"), but `encode_sxth` silently emits a 32-bit `SBFM` word for it.

The reverse direction (`sxth x0, w0`) is genuinely valid — llvm-mc even canonicalizes `sxth x0, x0` to `sxth x0, w0` — so the source width legitimately does not matter for a 64-bit destination. The bug is specifically the **32-bit destination paired with a 64-bit (X) source**, which has no encoding and should be rejected.

The sibling encoders `encode_sxtb`, `encode_uxth`, and `encode_uxtb` share the identical `let (rn, _) = get_reg(operands, 1)?;` pattern and therefore the same defect.

## Root Cause

```rust
pub(crate) fn encode_sxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- source width discarded
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`is_64` (and hence `sf`/`N`) is taken solely from the destination. The source register's `x`/`w` prefix is parsed only for its number. When the destination is `W` (sf=0) but the source is written as `Xn`, the encoder still produces a 32-bit word as though the source were `Wn`.

## Reproduction

Reference assembler (ground truth):
```
$ printf 'sxth w0, x0\n' | llvm-mc-18 --triple=aarch64
<stdin>:1:10: error: invalid operand for instruction
```

Failing property `sxth_props::sxth_rejects_w_destination_with_x_source`:
```
encode_sxth(&[Operand::Reg("w0"), Operand::Reg("x0")])
  => Ok(Word(0x13003C00))          // actual: silently encoded
  expected: Err                     // llvm-mc rejects this form
```
(0x13003C00 = the 32-bit `SBFM w0, w0, #0, #15` word.)

The provenance of the failing property's expectation is an `llvm-mc-18` run; the passing differential oracle (`sxth_reference_encoding`, `sxth_field_placement`) was likewise cross-checked against llvm-mc for both widths and the full register range.

## Impact

For an assembler backed by this encoder, the input `sxth w0, x1` is accepted and assembled into a valid-looking 32-bit instruction, masking what is almost certainly a programmer error (the author most likely intended a 64-bit destination or a 32-bit source). This diverges from GAS/llvm-mc behavior, so the same hand-written assembly can behave differently across assemblers. It is a correctness/robustness defect in operand validation rather than a mis-compilation of a legal instruction.

## Suggested Fix

Validate that the source register's width matches the destination before encoding, returning `Err` on mismatch (mirroring how `encode_sxtw` already pins the 64-bit destination, and how other encoders enforce width constraints):

```rust
pub(crate) fn encode_sxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, src_is_64) = get_reg(operands, 1)?;
    if src_is_64 && !is_64 {
        return Err("sxth: 64-bit source requires a 64-bit destination".to_string());
    }
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```
Note: a 64-bit destination with a 32-bit source (`sxth x0, w0`) must remain accepted, so the guard is one-directional. Apply the same fix to `encode_sxtb`, `encode_uxth`, and `encode_uxtb`.
