# Bug Report: `encode_cls` silently accepts mismatched register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_cls`
**Severity:** Medium (silent mis-encoding; assembler correctness contract violation)
**Status:** Confirmed by property-based test (expected failure).

## Summary

`encode_cls` derives the instruction's `sf` (operand-size) bit **only from the
destination register `Rd`**, and discards the source register `Rn`'s width:

```rust
pub(crate) fn encode_cls(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- Rn width discarded
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

As a result, mismatched-width operands such as `CLS X0, W0` or `CLS W0, X0`
are **silently accepted and encoded** using `Rd`'s width, producing a valid but
semantically wrong instruction word instead of an assembly error. This is the
exact same defect class as the already-reported `encode_clz`
(see `encode_clz-mismatched-register-widths.md`, which lists `encode_cls` as an
unfixed sibling).

## Specification

The ARM ARM defines CLS strictly as a same-size pair (Data-processing, 1 source):

- `CLS <Wd>, <Wn>`  →  `0 1 0 11010110 00000 000101 0 Rn Rd`
- `CLS <Xd>, <Xn>`  →  `1 1 0 11010110 00000 000101 0 Rn Rd`

There is a single shared `sf` field, so mixing `<W>` and `<X>` operands is
**not** a defined form and must be rejected by the assembler (cf. `as`/`llvm-mc`,
which diagnose `error: invalid operand for instruction`).

## Reproduction

Property `prop_rejects_mixed_width_operands` (expected-to-fail negative
contract) in module `prop_encode_cls_tests` fails on the minimal input:

```
CLS x0, w0 mixes widths and should be rejected, got Ok(Word(3670021120))
CLS w0, x0 mixes widths and should be rejected, got Ok(Word(1543503872))
minimal failing input: d = 0, n = 0
```

`3670021120 == 0xDAC01400`, i.e. the encoder emitted `CLS X0, X0`, completely
ignoring that the source operand was `W0`. The reverse case `CLS w0, x0`
emits `0x5AC01400` (`CLS W0, W0`).

```
$ cargo test --lib prop_encode_cls_tests
test ...::prop_cls_field_placement             ... ok    (sf/opcode/op2/Rn/Rd correct)
test ...::prop_cls_known_constants             ... ok    (CLS X0,X0=0xDAC01400; W0,W0=0x5AC01400)
test ...::prop_cls_xor_clz_is_only_bit_10      ... ok    (differential vs CLZ)
test ...::prop_width_changes_only_sf           ... ok    (x vs w differs only in sf)
test ...::prop_rejects_malformed_operands      ... ok    (missing / wrong-type slots → Err)
test ...::prop_rejects_mixed_width_operands    ... FAILED  (this finding)
```

## Verified correct behavior (the encoding itself is sound)

The structural encoding — `sf·1·0·11010110·00000·000101·Rn·Rd` — is correct:
`CLS X0,X0 = 0xDAC01400`, `CLS W0,W0 = 0x5AC01400`, opcode `[15:10]=000101`,
`op2 [20:16]=0`, and CLS differs from CLZ (`opcode 000100`) by **exactly bit 10**
(confirmed by the `CLS ^ CLZ` differential property). The field-placement,
known-constant, width-differential and sibling-differential properties all pass,
so the bug is purely the missing width-coherence validation, not a bit-layout
error.

## Related secondary observation: no operand-count validation

The malformed-operand property also exposed that trailing operands are silently
ignored: `CLS x0, x1, #0` (three operands) encodes successfully as `CLS X0, X1`
because the encoder only reads indices 0 and 1. A correct assembler should
reject a wrong operand count. This is noted here rather than tested as a
property to keep the must-fail negative contract focused on genuine type/arity
errors; it is the same "insufficient validation" theme.

## Suggested fix

Validate that source and destination register widths match before encoding:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, src_is_64) = get_reg(operands, 1)?;
if is_64 != src_is_64 {
    return Err("CLS requires source and destination registers of the same size".into());
}
```

The same discarded-width pattern (`let (rn, _)`) recurs across the bitfield /
data-processing (1 source) family — `encode_clz`, `encode_cls`, `encode_rbit`
(scalar), `encode_rev`, `encode_rev16`, `encode_rev32` — and should be fixed
consistently.
