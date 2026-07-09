# Bug Report: `encode_neon_umov` silently truncates lane index

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_umov`
**Severity:** High

## Summary

`encode_neon_umov` extracts lane index from arrangement string without range validation. NEON UMOV lane index varies by arrangement: 1-bit for `.b`, 2-bit for `.h`, 3-bit for `.s`, 4-bit for `.d`. Out-of-range indices accepted and silently modulo-encoded.

## Root Cause

```rust
let lane = arr_n.chars().nth_back(1).unwrap().to_digit(10).unwrap() as u32;  // no validation
```

## Reproduction

**Input:** `umov x0, v0.b[4]`

**Expected:** `Err` — NEON lane index 4 out of range for .b (valid: 0-1)

**Actual:** `Ok(Word(...))` — lane = 4 % 2 = 0, encoded as `[0]`

**Minimal failing input:** arr_n="b", lane=2 (or 4, 8, 127, etc.)

## Impact

Lane indices silently modulo-encoded. User expects operation at specific lane but gets different encoding.

## Suggested Fix

Validate index against arrangement:

```rust
let max = match arr_n.as_str() {
    "b" => 1, "h" => 3, "s" => 7, "d" => 15,
    _ => return Err(format!("unsupported arrangement: {}", arr_n)),
};
if lane > max {
    return Err(format!("lane index {} out of range for {}", lane, arr_n));
}
```

## Regression Property

Failing property: `neon_umov_lane_index_range_checked`

```rust
prop_assert!(encode_neon_umov(&[xreg(0), neon_reg(0, "b[4]")]).is_err());  // b: 0-1
prop_assert!(encode_neon_umov(&[xreg(0), neon_reg(0, "h[4]")]).is_err());  // h: 0-3
prop_assert!(encode_neon_umov(&[xreg(0), neon_reg(0, "s[8]")]).is_err());  // s: 0-7
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/89