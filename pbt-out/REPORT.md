# PBT Campaign Report: `src/ir/constants.rs`

## Summary

**Date:** 2026-07-07  
**Repository:** `/home/toan/evaluation/claudes-c-compiler`  
**Target:** `IrConst::cast_float_to_target`  
**Tests added:** 5  
**Result:** 4 passing, 1 failing property, 1 confirmed bug

## Modules Tested

| Module | Tests | Bugs | Oracles Used |
|--------|-------|------|--------------|
| `src/ir/constants.rs` | 5 | 1 | reference, equivalence |

## Bugs Found

- `IrConst::cast_float_to_target(..., IrType::F128)` panics for some `f64` inputs while constructing the long-double representation.
- Witness from the shrunk failing property: `v = -1.4205590610320785e-195`, `target = IrType::F128`.
- Reproducer: `cargo test -q --lib ir::constants::tests::cast_float_to_target_preserves_float_targets -- --nocapture`.
- Panic site: `src/common/long_double.rs:1040` (`attempt to subtract with overflow`).

## Design Caveats

- Pointer-width expectations are architecture-dependent because `ptr_int` intentionally returns `I32` on 32-bit targets and `I64` on LP64. Doc evidence: `src/ir/constants.rs:435`.
- The float-cast properties mirror the implementation's direct `as` conversions for integer and `F32` targets, while `F128` delegates to `IrConst::long_double`. Doc evidence: `src/ir/constants.rs:274`.

## Test Files Created

| File | Tests Added |
|------|-------------|
| `src/ir/constants.rs` | 5 |

## Output Directories

- `pbt-out/`
- `proptest-regressions/`
