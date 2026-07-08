# Bug Report — `encode_neg` silently accepts mixed register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_neg`
**Status:** Confirmed by failing property `neg_rejects_mixed_width_operands`.

## Summary

`encode_neg` derives `sf` only from `Rd` and discards the width flag returned for `Rm`. Mixed W/X operands are therefore accepted and encoded at the destination width instead of being rejected.

## Reproduction

```text
neg x0, w1
```

Actual behavior: returns `Ok` and encodes as the X form.

Expected behavior: return `Err` for operand-size mismatch.

## Impact

A source typo or macro-generated mixed-width NEG assembles successfully but uses a different register width than written.

## Suggested fix

Compare the `is_64` flags returned by `get_reg` for `Rd` and `Rm`; return `Err` when they differ.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/74
