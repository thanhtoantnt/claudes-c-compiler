# `encode_neon_ext` silently treats invalid arrangements as 8B

**Target:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_ext`

## Summary

`EXT` is valid only for `.8B` and `.16B` arrangements. The encoder computes `q` with `arr_d == "16b"` and treats every other arrangement as `Q=0`, so invalid arrangements like `.4s`, `.8h`, or `.2d` are silently encoded as the `.8b` form instead of returning `Err`.

## Reproduction

Failing property: `ext_range_pbt_tests::invalid_arrangement_must_be_rejected`

Minimal failing input:

```text
ext v0.4s, v1.4s, v2.4s, #0
```

Expected: `Err`, because EXT accepts only `.8b` and `.16b`.

Actual: `Ok(Word(_))`; `.4s` is treated as `.8b` by the `arr_d == "16b"` check.

## Impact

Invalid SIMD source text is accepted and encoded as a different instruction shape, hiding frontend/codegen bugs.

## Suggested fix

Reject any arrangement except `"8b"` and `"16b"` before computing `q`.
