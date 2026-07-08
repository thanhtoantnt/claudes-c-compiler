# Bug Report: `encode_movk` silently truncates out-of-range immediate magnitude

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movk`

## Summary

`encode_movk` masks the immediate with `& 0xFFFF` without validating that the source immediate fits in the 16-bit MOVK field. Out-of-range immediates are accepted and silently encoded as a different value.

## Reproduction

Failing property: `movk_rejects_out_of_range_immediate`

Minimal input:

```text
rd = 0, imm = 65536
```

`movk x0, #0x10000` encodes `imm16 = 0x10000 & 0xFFFF = 0x0000`, producing the same immediate field as `movk x0, #0x0` instead of returning `Err`.

## Impact

Silent miscompilation: constants assembled through MOVK can lose upper immediate bits with no diagnostic.

## Suggested fix

Validate `imm` before encoding:

```rust
if !(0..=0xFFFF).contains(&imm) {
    return Err(format!("movk immediate out of range: {}", imm));
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/59
