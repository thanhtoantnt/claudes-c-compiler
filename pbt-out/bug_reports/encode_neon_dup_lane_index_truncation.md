# Bug Report: `encode_neon_dup` silently truncates lane index

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_dup`
**Severity:** High

## Summary

`encode_neon_dup` extracts lane index from arrangement string without range validation. NEON lane index is 2-bit for `.8b` (0-3), 1-bit for `.16b` (0-1), 3-bit for `.4h` (0-7), 2-bit for `.8h` (0-3), 1-bit for `.2s` (0-1), 2-bit for `.4s` (0-3), 1-bit for `.2d` (0-1). Out-of-range indices accepted and silently modulo-encoded.

## Root Cause

```rust
let lane = arr_d.chars().nth_back(1).unwrap().to_digit(10).unwrap() as u32;  // no validation
```

## Reproduction

**Input:** `dup v0.8b, v1.8b[5]`

**Expected:** `Err` — NEON lane index 5 out of range for .8b (valid: 0-3)

**Actual:** `Ok(Word(...))` — lane = 5 % 4 = 1, encoded as `[1]`

**Minimal failing input:** arr_d="8b", lane=5 (or 4, 8, 127, etc.)

## Impact

Lane indices silently modulo-encoded. User expects operation at specific lane but gets different encoding.

## Suggested Fix

Validate index against arrangement:

```rust
let max = match arr_d.as_str() {
    "8b" => 3, "16b" => 1, "4h" => 7, "8h" => 3, "2s" => 1, "4s" => 3, "2d" => 1,
    _ => return Err(format!("unsupported arrangement: {}", arr_d)),
};
if lane > max {
    return Err(format!("lane index {} out of range for {}", lane, arr_d));
}
```

## Regression Property

Failing property: `neon_dup_lane_index_range_checked`

```rust
prop_assert!(encode_neon_dup(&[neon_reg(0, "8b"), neon_reg(1, "8b[5]")]).is_err());  // 8b: 0-3
prop_assert!(encode_neon_dup(&[neon_reg(0, "16b"), neon_reg(1, "16b[2]")]).is_err());  // 16b: 0-1
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/83