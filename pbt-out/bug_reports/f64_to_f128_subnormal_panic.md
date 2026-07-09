# Bug Report: f64_to_f128_bytes_lossless panics on subnormal f64 values

**Target:** `src/common/long_double.rs` → `f64_to_f128_bytes_lossless`
**Severity:** Medium

## Summary

`f64_to_f128_bytes_lossless()` in `src/common/long_double.rs:1040` performs unsigned subtraction `d.biased_exp as u128 - 1023` without checking for subnormals (biased_exp == 0). In debug mode this panics; in release mode it wraps to a huge exponent, producing a corrupt f128 encoding.

## Root Cause

Line 1040:
```rust
let exp15 = (d.biased_exp as u128 - 1023 + 16383) as u128;
```

For subnormals, `d.biased_exp == 0`. The function only checks `is_zero()` and `is_special()` before this line — it never handles the subnormal case (biased_exp == 0, mantissa != 0) which requires renormalization before computing the f128 exponent.

## Reproduction

**Input:** val = 5.45247436838069e-309 (subnormal: biased_exp=0, mantissa≠0)

**Expected:** `Some([u8; 16])` — successful f128 encoding

**Actual:** **Panic** — "attempt to subtract with overflow"

## Impact

Affects any compilation path that produces `long double` constants from subnormal `double` values. The compiler either panics (debug) or emits corrupt floating-point data (release).

## Suggested Fix

```rust
if d.biased_exp == 0 && d.mantissa != 0 {
    // Subnormal f64: renormalize by finding the leading 1 in the mantissa
    let shift = d.mantissa.leading_zeros() - (64 - 52);
    let normalized_mantissa = (d.mantissa << shift) & 0x000F_FFFF_FFFF_FFFF;
    let exp15 = (1u128 - 1023 + 16383 - shift as u128) as u128;
    // ... encode with renormalized mantissa and adjusted exponent
}
```

## Regression Property

```rust
proptest! {
    #[test]
    fn f64_to_f128_roundtrip(val in any::<f64>().prop_filter("finite", |v| v.is_finite())) {
        let bytes = f64_to_f128_bytes_lossless(val);
        let back = f128_bytes_to_f64(&bytes);
        prop_assert_eq!(val.to_bits(), back.to_bits());
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/113