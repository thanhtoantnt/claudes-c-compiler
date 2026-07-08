# Bug Report: `encode_sxtw` silently accepts a W destination register

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtw`
**Severity:** Medium (silent acceptance of an architecturally invalid instruction form; no diagnostic)

## Summary

`SXTW` (sign-extend word) is defined by the ARMv8 ARM **exclusively** as
`SXTW <Xd>, <Wn>` — a 64-bit destination is mandatory, because the whole point of
the instruction is 32-bit → 64-bit sign extension. There is no `SXTW <Wd>, <Wn>`
form. `encode_sxtw` never validates the destination width, so a malformed
`SXTW Wd, Wn` is silently accepted and emitted as a valid-looking `SBFM` word
with `sf=1` (i.e. the bytes of `SXTW Xd, Wn`), rather than rejected.

This was confirmed by **differential assembly** with clang/llvm-mc
(target `aarch64-linux-gnu`): `sxtw w0, w1` is rejected with
`error: invalid operand for instruction`, whereas the reference forms
`sxtw x0, w0`, `sxtw x5, w7`, `sxtw x30, w31` all assemble to
`0x93407C00 | (Rn<<5) | Rd` (e.g. `sxtw x0, w0` → little-endian `00 7c 40 93`).

## Root Cause

```rust
pub(crate) fn encode_sxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // SXTW Xd, Wn -> SBFM Xd, Xn, #0, #31
    let (rd, _) = get_reg(operands, 0)?;   // <-- `is_64` discarded: destination width never validated
    let (rn, _) = get_reg(operands, 1)?;   // <-- `is_64` discarded: source width never validated
    let word = ((1u32 << 31) | (0b100110 << 23) | (1 << 22)) | (31 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The `is_64` flag returned by `get_reg` is dropped for **both** operands (the
destination's flag is the relevant one here). `sf` is hardcoded to `1` via the
`1u32 << 31` literal, so a `W` destination produces the same bytes as the `X`
form rather than triggering an error. The dead destination-width parameter is
characterized directly by the passing property
`sxtw_props::sxtw_destination_width_is_ignored`, which shows that
`encode_sxtw([xreg(n), wreg(n)])` and `encode_sxtw([wreg(n), wreg(n)])` yield
byte-identical output.

## Reproduction

Property `sxtw_props::sxtw_rejects_w_destination` fails on the minimal input:

```
encode_sxtw(&[wreg(0), wreg(0)])   // "sxtw w0, w0"
  -> Ok(Word(2470476800))          // == 0x93407C00  (sf=1, as if "sxtw x0, w0")
  expected: Err(...)               // clang/llvm-mc: "invalid operand for instruction"
```

## Impact

- **Silent mis-encoding / no diagnostic.** A user typo such as `sxtw w0, w1`
  compiles to a real `SBFM x0, x0, #0, #31` instruction (writing a 64-bit `X0`)
  instead of being flagged. The destination register is reinterpreted as 64-bit,
  which corrupts the upper 32 bits of `X0` even though the source text named a
  32-bit `W0`.
- The whole purpose of `SXTW` (32→64 extension) is defeated when the
  destination is 32-bit; emitting valid-looking bytes hides a genuine source
  error.
- Consistent with other width-validation gaps in this encoder (see
  `encode_madd` / `encode_smulh` reports): the `is_64` flag from `get_reg` is
  frequently discarded.

## Suggested Fix

Validate that the destination is a 64-bit (`X`) register:

```rust
pub(crate) fn encode_sxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // SXTW Xd, Wn -> SBFM Xd, Xn, #0, #31
    let (rd, rd_is_64) = get_reg(operands, 0)?;
    if !rd_is_64 {
        return Err("sxtw requires a 64-bit destination register (Xd)".to_string());
    }
    let (rn, _) = get_reg(operands, 1)?;
    let word = ((1u32 << 31) | (0b100110 << 23) | (1 << 22)) | (31 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

(Optional: the source `Wn` width is also discarded. The ARMv8 ARM alias requires
`Wn`; clang/llvm-mc is lenient and accepts an `X` source, so a strict source
check is optional but recommended for full alias conformance.)

## Regression Property

Failing property: `sxtw_props::sxtw_rejects_w_destination`

```rust
#[test]
fn sxtw_rejects_w_destination(n in 0u32..=31) {
    let ops = vec![wreg(n), wreg(n)]; // sxtw w_n, w_n -- invalid destination
    prop_assert!(
        encode_sxtw(&ops).is_err(),
        "W destination is architecturally invalid for SXTW; got {:?}",
        encode_sxtw(&ops)
    );
}
```

Minimal failing input: `n = 0` → `encode_sxtw(&[wreg(0), wreg(0)])` returns
`Ok(Word(0x93407C00))`.
