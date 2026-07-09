# `encode_neon_ext` silently truncates EXT immediate indexes above 15

**Target:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_ext`

## Summary

`EXT` has a 4-bit immediate field. The encoder masks the parsed index with `index & 0xF`, so out-of-range indexes such as `#16` and `#17` are silently re-encoded as `#0` and `#1` instead of returning `Err`.

## Reproduction

Failing property: `ext_range_pbt_tests::out_of_range_imm4_must_be_rejected`

Minimal failing input:

```text
ext v0.16b, v1.16b, v2.16b, #16
```

Expected: `Err`, because the EXT immediate must be in `0..=15`.

Actual: `Ok(Word(_))`; the immediate is masked to zero.

## Impact

A typo or codegen bug changes the selected byte offset without diagnostic, producing wrong vector data.

## Suggested fix

Validate the immediate before encoding: reject values outside `0..=15` and remove the masking-as-validation behavior.
