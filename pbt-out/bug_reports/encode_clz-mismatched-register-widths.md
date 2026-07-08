# Bug Report: `encode_clz` silently accepts mismatched register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_clz`
**Severity:** Medium (silent mis-encoding; assembler correctness contract violation)
**Status:** Confirmed by property-based test (expected failure).

## Summary

`encode_clz` derives the instruction's `sf` (operand-size) bit **only from the
destination register `Rd`**, and discards the source register `Rn`'s width:

```rust
pub(crate) fn encode_clz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- Rn width discarded
    let sf = sf_bit(is_64);
    ...
}
```

As a result, mismatched-width operands such as `CLZ X0, W0` or `CLZ W0, X0`
are **silently accepted and encoded** using `Rd`'s width, producing a valid but
semantically wrong instruction word instead of an assembly error.

## Specification

The ARM ARM defines CLZ strictly as a same-size pair:

- `CLZ <Wd>, <Wn>`
- `CLZ <Xd>, <Xn>`

Mixing `<W>` and `<X>` operands is **not** a defined form and must be rejected
by the assembler (cf. `as`/`llvm-mc`, which diagnose
`error: invalid operand for instruction`).

## Reproduction

Property `prop_rejects_mismatched_register_widths` (expected-to-fail negative
contract) in module `prop_encode_clz_tests` fails on the minimal input:

```
CLZ x0, w0 (mismatched widths) should be Err, got Ok(Word(3670020096))
```

`3670020096 == 0xDAC01000`, i.e. the encoder emitted `CLZ X0, X0`, completely
ignoring that the source operand was `W0`.

```
$ cargo test --lib prop_encode_clz
test ...::prop_clz_field_placement             ... ok   (sf/Rn/Rd correct, N=1 fixed)
test ...::prop_clz_xor_cls_is_only_bit_10      ... ok   (differential vs CLS)
test ...::prop_clz_xor_rbit_is_only_bit_12     ... ok   (differential vs RBIT)
test ...::prop_width_changes_only_sf           ... ok   (x vs w differs only in sf)
test ...::prop_rejects_malformed_operands      ... ok
test ...::prop_rejects_mismatched_register_widths ... FAILED  (this finding)
```

## Verified correct behavior (the encoding itself is sound)

The structural encoding — `sf·1·0·1101011·000000·000100·Rn·Rd` — is correct:
`CLZ X0,X0 = 0xDAC01000`, `CLZ W0,W0 = 0x5AC01000`, opcode `000100` (=4),
opcode2 `[21:16]=0`, and bit[22] ("N") correctly **fixed to 1** (NOT equal to
`sf`, unlike the UBFM/SBFM/BFM bitfield family). The bit-placement, width
differential, and sibling-differential (CLS / RBIT) properties all pass.

## Suggested fix

Validate that source and destination register widths match before encoding:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, src_is_64) = get_reg(operands, 1)?;
if is_64 != src_is_64 {
    return Err("CLZ requires source and destination registers of the same size".into());
}
```

The same discarded-width pattern (`let (rn, _)`) recurs in the sibling
encoders `encode_cls`, `encode_rbit` (scalar), `encode_rev`, `encode_rev16`,
`encode_rev32` and should be fixed consistently.
