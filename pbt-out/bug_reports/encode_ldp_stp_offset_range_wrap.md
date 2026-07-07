# Bug Report: `encode_ldp_stp` silently wraps out-of-range pair offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldp_stp`

## Summary

`encode_ldp_stp` masks the scaled pair offset with `& 0x7F` instead of validating the signed 7-bit immediate range. Out-of-range offsets are accepted and encoded as a different in-range offset.

## Reproduction

Failing property: `prop_out_of_range_offset_is_rejected`

Minimal input from the run:

```text
ldp x0, x1, [x2, #505]
```

For 64-bit general-purpose pair loads/stores, the offset is scaled by 8 and encoded in signed imm7, so the valid byte range is `[-512, 504]`. Offset `505` is outside the range and should return `Err`. The encoder instead masks the shifted value with `& 0x7F`, silently producing a nearby/different offset encoding.

## Impact

Silent miscompilation: stack or memory pair operations can access a different address than the assembly source requested, with no diagnostic.

## Suggested fix

Before encoding, check that the offset is in the scaled signed imm7 range for the register class/element size:

```rust
if offset < -(64 * scale) || offset > (63 * scale) {
    return Err(format!("ldp/stp offset out of range: {}", offset));
}
```

Then encode only after validation.
