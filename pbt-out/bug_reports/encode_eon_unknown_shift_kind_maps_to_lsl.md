# Bug Report — `encode_eon` treats unknown shift kinds as LSL

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_eon`
**Status:** Confirmed by failing property `eon_unknown_shift_kind_is_rejected`.

## Summary

`encode_eon` maps shift kinds with a catch-all arm that returns the LSL encoding (`0b00`) for any unrecognized kind. Unknown shift kinds must be rejected instead of being silently coerced to `lsl`.

## Reproduction

```text
eon x0, x1, x2, foo #1
```

Actual behavior: returns `Ok`, encoding the shift as `lsl #1`.

Expected behavior: return `Err` for an unknown shift kind.

## Impact

Typos or parser bugs in the shift kind silently produce a valid but unintended instruction.

## Suggested fix

Replace the catch-all shift mapping with an error for unknown kinds, as done by stricter sibling encoders.

## Regression property

Failing property: `eon_unknown_shift_kind_is_rejected`

```rust
prop_assert!(encode_eon(&[xreg(0), xreg(1), xreg(2)], "foo", 1).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/41
