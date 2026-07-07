# Bug Report: `encode_eon` silently accepts W-register shifts above 31

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_eon`

## Summary

`encode_eon` masks the shift amount with `& 0x3F` and does not validate the width-dependent limit. For 32-bit W-register forms, ARM permits shift amounts only in `0..=31`; amounts `32..=63` are accepted and encoded as UNPREDICTABLE instructions.

## Reproduction

Failing property: `eon_w_register_rejects_shift_above_31`

Minimal input:

```text
eon w0, w0, w0, lsl #32
```

Actual result: `Ok(Word(_))` instead of `Err`.

## Impact

The assembler accepts invalid EON shifted-register source and emits an undefined 32-bit instruction encoding.

## Suggested fix

Check the register width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("eon shift out of range: {}", shift_amount));
}
```
