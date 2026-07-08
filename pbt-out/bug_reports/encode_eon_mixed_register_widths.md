# Bug Report — `encode_eon` silently accepts mixed register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_eon`
**Status:** Confirmed by failing property `eon_rejects_mixed_register_widths`.

## Summary

`encode_eon` derives `sf` from the destination operand and discards the width flags for source operands. Mixed W/X operands are therefore accepted and encoded using the destination width.

## Reproduction

```text
eon w0, x1, w2
```

Actual behavior: returns `Ok` instead of rejecting the operand-size mismatch.

Expected behavior: return `Err`; scalar EON operands must have one shared W/X width.

## Impact

A mixed-width source typo silently assembles into an instruction operating at a different width than the source text names.

## Suggested fix

Compare the `is_64` flags returned by `get_reg` for `Rd`, `Rn`, and `Rm`; reject when they differ.

## Regression property

Failing property: `eon_rejects_mixed_register_widths`

```rust
prop_assert!(encode_eon(&[wreg(0), xreg(1), wreg(2)], "lsl", 0).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/39
