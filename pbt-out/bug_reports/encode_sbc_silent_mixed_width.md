# Bug Report — `encode_sbc` silently accepts mismatched operand widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_sbc`
**Status:** Confirmed by failing property `sbc_rejects_mixed_width_operands`.

## Summary

`encode_sbc` derives the instruction width (`sf`, bit 31) only from the destination operand `Rd`. The source operand widths from `Rn` and `Rm` are discarded by `let (rn, _) = get_reg(...)` / `let (rm, _) = ...`. Mixed W/X operands are therefore silently encoded using the destination width.

## Reproduction

Failing property: `sbc_rejects_mixed_width_operands`

Minimal example:

```text
sbc x0, w1, x2
```

Actual behavior: returns `Ok(Word(_))`, encoding the source register number as if it were `x1`.

Expected behavior: return `Err` for operand-size mismatch.

## Impact

A typo or macro-generated mixed-width `SBC`/`SBCS` instruction assembles without error but performs an operation at the destination width, reading a different architectural register view than the source text names.

## Suggested fix

Preserve and compare the `is_64` flags returned by `get_reg` for `Rd`, `Rn`, and `Rm`; reject when they differ before emitting the word.
