# Bug Report: `encode_sxtb` silently accepts architecturally invalid mixed-width source operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtb`
**Severity:** Medium (silent acceptance of an invalid instruction form; diverges from the reference assembler)

## Summary

`encode_sxtb` derives the operation width *only* from the destination register (`get_reg(operands, 0)`) and ignores the width of the source register `Rn`. Per the ARMv8 ARM, `SXTB` is an alias of `SBFM`, whose encoding requires the `N` bit to equal `sf` — i.e. the source and destination registers must be the same width. The reference assembler (llvm-mc-18, triple `aarch64`) therefore rejects `sxtb w0, x0` ("error: invalid operand for instruction"), but `encode_sxtb` silently emits a 32-bit `SBFM` word for it.

The reverse direction (`sxtb x0, w0`) is genuinely valid — llvm-mc even canonicalizes `sxtb x0, x0` to `sxtb x0, w0` (both → `0x93401c00`) — so the source width legitimately does not matter for a 64-bit destination. The bug is specifically the **32-bit destination paired with a 64-bit (X) source**, which has no encoding and should be rejected.

The sibling encoders `encode_sxth`, `encode_uxth`, and `encode_uxtb` share the identical `let (rn, _) = get_reg(operands, 1)?;` pattern and therefore the same defect (see `encode_sxth_silent_mixed_width_source.md`).

## Root Cause

```rust
pub(crate) fn encode_sxtb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- source width discarded
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (7 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`is_64` (and hence `sf`/`N`) is taken solely from the destination. The source register's `x`/`w` prefix is parsed only for its number. When the destination is `W` (sf=0) but the source is written as `Xn`, the encoder still produces a 32-bit word as though the source were `Wn`.

## Reproduction

Reference assembler (ground truth):
```
$ printf 'sxtb w0, x0\n' | llvm-mc-18 --triple=aarch64
<stdin>:1:10: error: invalid operand for instruction
```

Failing property `sxtb_props::sxtb_rejects_w_destination_with_x_source`:
```
encode_sxtb(&[Operand::Reg("w0"), Operand::Reg("x0")])
  => Ok(Word(0x13001C00))          // actual: silently encoded
  expected: Err                     // llvm-mc rejects this form
```
(0x13001C00 = the 32-bit `SBFM w0, w0, #0, #7` word; `318774272` decimal as printed by the shrunk property.)

The provenance of the failing property's expectation is an `llvm-mc-18` run; the passing differential oracle (`sxtb_reference_encoding`, `sxtb_field_placement`) was likewise cross-checked against llvm-mc for both widths and the full register range (`sxtb x0,w0`→`0x93401c00`, `sxtb w5,w7`→`0x13001ce5`, `sxtb xzr,wzr`→`0x93401fff`).

## Impact

For an assembler backed by this encoder, the input `sxtb w0, x1` is accepted and assembled into a valid-looking 32-bit instruction, masking what is almost certainly a programmer error (the author most likely intended a 64-bit destination or a 32-bit source). This diverges from GAS/llvm-mc behavior, so the same hand-written assembly can behave differently across assemblers. It is a correctness/robustness defect in operand validation rather than a mis-compilation of a legal instruction.

## Suggested Fix

Validate that the source register's width matches the destination before encoding, returning `Err` on mismatch (mirroring how `encode_sxtw` already pins the 64-bit destination, and how other encoders enforce width constraints):

```rust
pub(crate) fn encode_sxtb(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, src_is_64) = get_reg(operands, 1)?;
    if src_is_64 && !is_64 {
        return Err("sxtb: 64-bit source requires a 64-bit destination".to_string());
    }
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };
    let word = ((sf << 31) | (0b100110 << 23) | (n << 22)) | (7 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```
Note: a 64-bit destination with a 32-bit source (`sxtb x0, w0`) must remain accepted, so the guard is one-directional. Apply the same fix to `encode_sxth`, `encode_uxth`, and `encode_uxtb`.

## Regression Property

Failing property: `sxtb_props::sxtb_rejects_w_destination_with_x_source`

```rust
#[test]
fn sxtb_rejects_w_destination_with_x_source(n in 0u32..=31) {
    let ops = vec![wreg(n), xreg(n)]; // sxtb wN, xN -- invalid
    prop_assert!(
        encode_sxtb(&ops).is_err(),
        "W destination + X source is architecturally invalid for SXTB; got {:?}",
        encode_sxtb(&ops)
    );
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/99
