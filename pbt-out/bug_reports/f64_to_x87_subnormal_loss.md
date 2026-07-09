# Bug Report: f64_to_x87_bytes_simple loses subnormal f64 values (decodes as 0)

**Target:** `src/common/long_double.rs` → `f64_to_x87_bytes_simple`
**Severity:** Medium

## Summary

`f64_to_x87_bytes_simple()` in `src/common/long_double.rs` encodes a subnormal f64 value into x87 80-bit bytes, but `x87_bytes_to_f64()` decodes it back as `0.0` — the subnormal is silently dropped.

## Root Cause

The x87 encoder handles the subnormal case by shifting the mantissa and adjusting the exponent, but the decoder does not correctly reconstruct the subnormal from the x87 representation — it treats the small exponent as zero and returns 0.0. Alternatively, the encoder may not correctly set the explicit integer bit (J-bit) for the x87 subnormal representation, causing the decoder to interpret the value as an x87 pseudo-denormal or unnormal (which maps to zero in IEEE semantics).

## Reproduction

**Input:** val = 5.45247436838069e-309 (subnormal: biased_exp=0, mantissa≠0)

**Expected:** `Some(bytes16)` — encodes correctly, roundtrip succeeds

**Actual:** Encodes but decodes back as 0.0 — subnormal silently lost

## Impact

Affects compilation paths that produce `long double` constants from very small (subnormal) `double` values. The compiler silently compiles a non-zero subnormal constant as 0.0 in long-double form.

## Regression Property

```rust
proptest! {
    #[test]
    fn f64_to_x87_roundtrip(val in any::<f64>().prop_filter("finite", |v| v.is_finite())) {
        let bytes16 = f64_to_x87_bytes_simple(val);
        let back = x87_bytes_to_f64(&bytes16);
        prop_assert_eq!(val.to_bits(), back.to_bits());
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/114