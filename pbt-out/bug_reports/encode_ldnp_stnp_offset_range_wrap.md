# Bug Report: `encode_ldnp_stnp` silently wraps out-of-range imm7 offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldnp_stnp`

## Summary

`encode_ldnp_stnp` encodes the non-temporal pair offset as `(*offset >> shift) & 0x7F` without validating the signed 7-bit scaled range. Out-of-range byte offsets are accepted and wrap to a different signed imm7 value.

## Reproduction

Failing property: `prop_negative_imm7_range_violation_rejects`

Minimal input from the run:

```text
stnp w0, w1, [x2, #256]
```

For W-register non-temporal pairs, the offset is scaled by 4 and encoded in signed imm7, so the valid byte range is `[-256, 252]`. Offset `#256` is outside the range and should return `Err`. The encoder masks it into `imm7 = -64`, which decodes as `#-256`.

## Impact

Silent miscompilation: an out-of-range non-temporal pair load/store can access a different address from the one written in assembly.

## Suggested fix

Before masking, validate the signed scaled imm7 range for the access size:

```rust
if offset < -(64 * scale) || offset > (63 * scale) {
    return Err(format!("ldnp/stnp offset out of range: {}", offset));
}
```
