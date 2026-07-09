# Bug: `encode_fmov` accepts GP/FP register width mismatches

## Summary
FMOV between GP and FP registers requires matching widths: `fmov Dd, Xn` (64-bit) or `fmov Sd, Wn` (32-bit). The encoder silently accepts mismatched operands like `fmov Dd, Wn` or `fmov Sd, Xn`, producing instruction words with wrong `sf`/`ftype` field combinations that are either UNALLOCATED or encode a different instruction.

## Witness
```
cargo test -- --ignored prop_fmov_rejects_gp_fp_width_mismatch
```

## Root cause
No cross-validation between GP register width and FP register precision in the GP↔FP transfer path.

## Severity
MEDIUM — silent misencoding; produces UNALLOCATED or wrong instruction.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/271
