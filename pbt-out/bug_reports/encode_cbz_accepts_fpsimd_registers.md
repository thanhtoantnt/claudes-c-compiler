# Bug: `encode_cbz` silently accepts FP/SIMD register operands

## Summary
CBZ/CBNZ are GP-only instructions. The encoder accepts FP/SIMD register names (v0, d0, s0, etc.) via `parse_reg_num`, silently encoding e.g. `cbz v0, label` identically to `cbz w0, label`. `clang` rejects `cbz d0, label`.

## Witness
```
cargo test --lib div_tst_cbz_regclass_pbt -- --ignored prop_cbz_rejects_fpsimd_register_class
```

## Root cause
`parse_reg_num` accepts FP/SIMD prefixes and treats non-'x' prefixed names as 32-bit (sf=0), mapping them to GP register numbers without class validation.

## Severity
MEDIUM — silent misencoding; accepts invalid assembly that produces correct-looking but wrong instruction words.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/340
