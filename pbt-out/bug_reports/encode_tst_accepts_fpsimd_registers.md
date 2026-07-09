# Bug: `encode_tst` silently accepts FP/SIMD register operands

## Summary
TST (ANDS) is a GP-only instruction. The encoder accepts FP/SIMD register names (v0..v31, d0..d31, s0..s31) via `parse_reg_num`, silently encoding them as their GP namesakes. `clang` rejects `tst v0, x1`.

## Witness
```
cargo test --lib div_tst_cbz_regclass_pbt -- --ignored prop_tst_rejects_fpsimd_register_class
```

## Root cause
`parse_reg_num` accepts FP/SIMD prefixes and maps them to the numeric register ID without validating the register class.

## Severity
MEDIUM — silent misencoding; accepts invalid assembly that should be rejected.
