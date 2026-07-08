# Bug Report: `encode_ldnp_stnp` silently truncates unaligned offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldnp_stnp`

## Summary

`encode_ldnp_stnp` scales byte offsets with a right shift before encoding. It does not check that the original byte offset is aligned to the access size, so unaligned offsets are silently rounded down to a different aligned address.

## Reproduction

Failing property: `prop_negative_misaligned_offset_rejects`

Minimal input from the run:

```text
ldnp w0, w1, [x2, #5]
```

For W-register non-temporal pairs, offsets must be multiples of 4. Offset `#5` should return `Err`. The encoder shifts it right by 2, discarding the low bits and effectively encoding `#4`.

## Impact

Silent miscompilation: invalid unaligned source offsets assemble to a different aligned offset without any diagnostic.

## Suggested fix

Before scaling, require alignment:

```rust
if offset % scale != 0 {
    return Err(format!("ldnp/stnp offset must be aligned to {} bytes: {}", scale, offset));
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/45
