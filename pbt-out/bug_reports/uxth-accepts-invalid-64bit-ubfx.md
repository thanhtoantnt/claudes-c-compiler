# BUG: `encode_uxth` accepts architecturally-invalid 64-bit form, emits UBFX

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxth`
**Severity:** Correctness (silent mis-assembly / undefined encoding acceptance)
**Status:** Confirmed by failing property `uxth_props::uxth_rejects_64bit_destination_form`

## Summary

`encode_uxth` derives the operand width from the destination register only and
blindly emits a `UBFM`-family word for the 64-bit case. Per the ARMv8 ARM,
**UXTH is a 32-bit-only alias** of `UBFM`. The 64-bit destination form
`uxth <Xd>, <Xn>` has **no valid encoding** and is unconditionally rejected by
the reference assembler. This encoder instead silently accepts it and produces a
word that decodes as a *different* instruction (`UBFX`).

## Root cause

```rust
pub(crate) fn encode_uxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // <-- width taken from destination only
    let (rn, _) = get_reg(operands, 1)?;        // <-- source width discarded
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0 };       // <-- for 64-bit, N=1 => UBFX, not UXTH
    let word = ((sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22))
             | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

There is no width check. When the destination is `xN`, `is_64` becomes true,
`sf=1` and `N=1`, yielding `0xD3403C00 | (rn<<5) | rd`.

## Evidence — differential test against `llvm-mc-18`

```
$ echo 'uxth x0, x0' | llvm-mc-18 --triple=aarch64
<stdin>:1:10: error: invalid operand for instruction

$ echo 'uxth w0, w0' | llvm-mc-18 --triple=aarch64 --show-encoding
        uxth    w0, w0   // encoding: [0x00,0x3c,0x00,0x53]   == 0x53003C00  (valid)
```

The encoder's 64-bit output `0xD3403C00` disassembles as:
```
ubfx x0, x0, #0, #16      (== UBFM x0, x0, #0, #15)
```
i.e. a valid instruction but **not** UXTH — the assembler has silently
substituted one instruction for another based on an input the spec forbids.

## Failing property

`uxth_props::uxth_rejects_64bit_destination_form` (P6):

```
minimal failing input: rd = 0, rn = 0
UXTH is 32-bit-only; `uxth x0, x0` is architecturally invalid but got Ok(Word(3544202240))
```

`3544202240 == 0xD3403C00`.

The other 5 properties pass and pin the correct 32-bit behaviour:
- `uxth_reference_encoding_32bit` → `0x53003C00 | (rn<<5) | rd`
- `uxth_field_placement` → `sf=0, opc=10, fixed=100110, N=0, immr=0, imms=15`
- `uxth_source_width_irrelevant_for_32bit_destination`
- `uxth_rejects_too_few_operands`
- `uxth_rejects_non_register_operands`

## Suggested fix

Reject any destination that is not a 32-bit (`W`) register:

```rust
pub(crate) fn encode_uxth(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    if is_64 {
        return Err("uxth requires a 32-bit (W) destination register".to_string());
    }
    let (rn, _) = get_reg(operands, 1)?;
    // sf=0, opc=10, 100110, N=0, immr=0, imms=15
    let word = (0b10 << 29) | (0b100110 << 23) | (15 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Related

Sibling extend/extract encoders `encode_sxtb`, `encode_sxth`, `encode_uxtb`
share the same "no width validation" pattern (their existing `*_props` modules
already document the W-destination/X-source mismatch as a SPEC BUG). For those,
64-bit destinations *are* spec-valid (sign/zero-extend into 64-bit); for
`encode_uxth` the entire 64-bit path is invalid.
## Regression Property

Failing property: `uxth_rejects_64bit_destination_form` (P6)

```rust
prop_assert!(encode_uxth(&[xreg(0), xreg(0)]).is_err());  // X destination invalid
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/126
