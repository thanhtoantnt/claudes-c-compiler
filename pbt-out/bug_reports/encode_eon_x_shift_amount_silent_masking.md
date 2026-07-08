# Bug Report — `encode_eon` silently masks X-register shift amounts above 63

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_eon`
**Status:** Confirmed by failing property `eon_x_register_shift_above_63_is_rejected`.

## Summary

`encode_eon` masks the shifted-register amount with `& 0x3F` instead of validating the architectural range. For 64-bit EON, valid shifts are `0..=63`; amounts above 63 must be rejected.

## Reproduction

```text
eon x0, x1, x2, lsl #64
```

Actual behavior: returns `Ok`, encoding `lsl #0` because `64 & 0x3F == 0`.

Expected behavior: return `Err` for an out-of-range shift.

## Impact

Invalid assembly is accepted and silently re-encoded as a different valid instruction, changing the operation with no diagnostic.

## Suggested fix

Validate the shift amount before masking: `amount <= 63` for X-register forms and `amount <= 31` for W-register forms.
