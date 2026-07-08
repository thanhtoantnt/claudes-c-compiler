# Bug Report: `encode_uxtw` emits `MOV Wd, Wn` instead of canonical `UBFM` for the valid form

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxtw`
**Severity:** High

## Summary

`encode_uxtw` emits the encoding of `MOV Wd, Wn` (i.e. `ORR Wd, WZR, Wn` =
`0x2A0003E0 | (rn << 16) | rd`) instead of the canonical `UXTW` encoding for the
only spec-valid standalone form `uxtw Xd, Wn`. The ARMv8 ARM defines no standalone
`UXTB`/`UXTH`-style alias for `uxtw`; the reference assembler encodes the valid
form as `UBFM`:

```
uxtw Xd, Wn   ->   UBFM Xd, Xn, #0, #31   (zero-extend word -> doubleword)
```

The emitted word is a *32-bit* `ORR Wd, WZR, Wn`, which is structurally different
from the sibling encoders `encode_uxtb` / `encode_uxth` (both correctly emit the
canonical `UBFM` alias).

## Root Cause

```rust
pub(crate) fn encode_uxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // UXTW is MOV Wd, Wn (the upper 32 bits are zeroed)
    // Or: UBFM Xd, Xn, #0, #31
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    // Use 32-bit ORR (MOV alias)
    let word = (0b001010100 << 23) | (rn << 16) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The code's own comment records the correct form (`UBFM Xd, Xn, #0, #31`) but the
implementation chose the wrong branch, emitting the 32-bit ORR/MOV constant
`0x2A0003E0` with the source operand placed in the `Rm` field.

## Reproduction

Differential check against `llvm-mc-18 --triple=aarch64 --show-encoding`:

```
$ echo 'uxtw x0, w1' | llvm-mc-18 --triple=aarch64 --show-encoding
  ubfx x0, x1, #0, #32   // encoding: [0x20,0x7c,0x40,0xd3]
=> expected 0xD3407C00 | (rn << 5) | rd
```

- **input:** `uxtw x0, w1` → `ops = [Reg("x0"), Reg("w1")]`
- **actual:** `Ok(EncodeResult::Word(0x2A0103E0))` (= `0x2A0003E0 | (1<<16) | 0`,
  a 32-bit `mov w0, w1`; cross-check: `mov w0, w1` → `0x2A0103E0`)
- **expected:** `Ok(EncodeResult::Word(0xD3407C20))` (= `0xD3407C00 | (1<<5) | 0`,
  `UBFM x0, x1, #0, #31`)

The emitted word neither preserves the 64-bit destination nor matches any valid
`uxtw` encoding.

## Impact

Code emitted for any `uxtw Xd, Wn` source operand is wrong. Downstream consumers
(the disassembler/linker, or humans reading object dumps) see a 32-bit `mov`
where the source said `uxtw`, and the 64-bit word→doubleword zero-extension
semantics are lost. Because `uxtw` is the canonical operand-extension used for
array indexing / pointer arithmetic in many calling sequences, mis-emitting it as
a 32-bit `mov` corrupts address computation.

## Suggested Fix

Emit the canonical `UBFM Xd, Xn, #0, #31`:

```rust
pub(crate) fn encode_uxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // must be 64-bit (Xd)
    let (rn, _) = get_reg(operands, 1)?;   // source Wn
    // UBFM Xd, Xn, #0, #31: 1 10 100110 1 000000 011111 Rn Rd
    let word = 0xD3407C00u32 | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Regression Property

Failing property: `uxtw_canonical_encoding_for_xd_wn` (mod `uxtw_props`,
`data_processing.rs:7642`).

```rust
#[test]
fn uxtw_canonical_encoding_for_xd_wn() {
    let ops = vec![xreg(0), wreg(1)]; // uxtw x0, w1 -- the valid form
    let w = expect_word(encode_uxtw(&ops));
    assert_eq!(w, 0xD3407C00u32 | (1 << 5) | 0); // expected UBFM x0, x1, #0, #31
}
```

**Reproduce:** `cargo test --lib data_processing::uxtw_props::uxtw_canonical_encoding_for_xd_wn`
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/111
