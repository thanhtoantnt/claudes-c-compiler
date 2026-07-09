# `encode_neon_ext` accepts undefined 8B high-index encodings

**Target:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_ext`

## Summary

For `EXT Vd.8B, Vn.8B, Vm.8B, #index`, the ARM encoding uses `Q=0` and requires `imm4<3> == 0`, so valid indexes are only `0..=7`. The encoder accepts indexes `8..=15` for `.8b` and emits architecturally undefined encodings.

## Reproduction

Failing property: `ext_range_pbt_tests::ext_8b_high_index_must_be_rejected`

Minimal failing input:

```text
ext v0.8b, v1.8b, v2.8b, #8
```

Expected: `Err`, because `.8b` only supports indexes `0..=7`.

Actual: `Ok(Word(_))`; the high index is encoded into `imm4` with `Q=0`.

## Impact

The assembler emits an undefined vector instruction instead of rejecting invalid source text.

## Suggested fix

After validating `index <= 15`, add a `.8b`-specific guard rejecting `index > 7`.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/176
