# BUG: cast_float_to_target panics on small f64 values when target is F128

## Summary

`IrConst::cast_float_to_target(val, IrType::F128)` panics with "attempt to
subtract with overflow" for certain small f64 values. The panic is in
`f64_to_f128_bytes_lossless` (line 1040) — the same subnormal handling bug
found independently in the manual PBT suite.

This confirms the bug is **reachable from real compiler operations** (not just
a standalone helper test): any C code that casts a small `double` constant to
`long double` triggers this path.

## Witness (shrunk by proptest)

```
v = -1.4205590610320785e-195
target = IrType::F128
```

## Reproduction

```sh
cargo test --lib ir::constants::tests::cast_float_to_target_preserves_float_targets
```

## Root cause

Same as `f64_to_f128_subnormal_panic.md`: `f64_to_f128_bytes_lossless` at
line 1040 computes `d.biased_exp as u128 - 1023` without handling the case
where `biased_exp == 0` (subnormal f64). The unsigned subtraction overflows.

Call chain: `cast_float_to_target(val, F128)` → `IrConst::long_double(val)` →
`f64_to_f128_bytes_lossless(val)` → panic.

## Severity

Medium-High — affects **any compilation** that produces a `long double` constant
from a small `double` value (e.g. `long double x = 1e-310;`). The compiler
panics instead of producing an executable.

## Property that found it

```rust
proptest! {
    #[test]
    fn cast_float_to_target_preserves_float_targets(v in any::<f64>(), choice in 0u8..3) {
        let target = match choice { 0 => IrType::F32, 1 => IrType::F64, _ => IrType::F128 };
        let result = cast_float_to_target(v, target);
        // For float targets, cast should always succeed (never None)
        prop_assert!(result.is_some(), "cast to {:?} returned None for {}", target, v);
    }
}
```

## Found by

pi-pbt autonomous run (function-scoped prompt, 7 min, 56 turns).
