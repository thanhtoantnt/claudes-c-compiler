# Bug Report: `encode_ldp_stp` silently rounds unaligned pair offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldp_stp`

## Summary

`encode_ldp_stp` scales offsets with an arithmetic right shift before encoding. It does not require the byte offset to be aligned to the access size. Unaligned offsets are accepted and rounded/floored to another representable offset instead of returning `Err`.

## Reproduction

Failing property: `prop_unaligned_offset_is_rejected`

Minimal input from the run:

```text
ldp x0, x1, [x2, #9]
```

For 64-bit general-purpose pair loads/stores, offsets must be multiples of 8. Offset `9` should be rejected. The encoder shifts it right by 3, discarding the remainder and effectively encoding offset `8`.

## Impact

Silent miscompilation: an invalid unaligned pair load/store assembles to a different aligned address without any diagnostic.

## Suggested fix

Before scaling, check alignment:

```rust
if offset % scale != 0 {
    return Err(format!("ldp/stp offset must be aligned to {} bytes: {}", scale, offset));
}
```

Then perform the signed-range check and encode the scaled imm7 field.
