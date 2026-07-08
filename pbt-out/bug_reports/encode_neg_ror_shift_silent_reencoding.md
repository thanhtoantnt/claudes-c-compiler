# Bug Report — `encode_neg` silently re-encodes `ror` as `lsl`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_neg`
**Status:** Confirmed by failing property `neg_rejects_ror_shift`.

## Summary

`encode_neg` maps any unrecognized shift kind to `lsl` via the default arm of a `match`. The ARMv8 scalar add/sub shifted-register forms only permit `lsl`, `lsr`, and `asr`; `ror` is not valid here and must be rejected.

## Reproduction

```text
neg x0, x1, ror #5
```

Actual behavior: returns `Ok` and encodes the shift as `lsl #5`.

Expected behavior: return `Err` for the unsupported shift kind.

## Impact

Typos or upstream parser bugs silently produce a different instruction than the assembly source names.

## Suggested fix

Return `Err` for any shift kind other than `lsl`, `lsr`, or `asr`.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/75
