# Bug Report — `encode_neg` silently truncates W-register shift amounts above 31

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_neg`
**Status:** Confirmed by failing property `neg_w_reg_rejects_shift_above_31`.

## Summary

`encode_neg` masks the shift amount with `& 0x3F` instead of validating the architecture limit. For 32-bit operands, shift amounts must be `0..=31`; larger values are undefined and must be rejected.

## Reproduction

```text
neg w0, w1, lsl #32
```

Actual behavior: returns `Ok` and encodes a reserved/undefined form.

Expected behavior: return `Err`.

## Impact

A bad shift amount silently assembles into a different instruction, changing program semantics without a diagnostic.

## Suggested fix

Validate shift range against operand width before encoding (`31` for W, `63` for X).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/78
