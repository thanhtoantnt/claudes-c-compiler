# Bug Report: `encode_neon_ins` silently truncates lane index

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ins`
**Severity:** High

## Summary

`encode_neon_ins` extracts lane index from arrangement string without range validation. NEON INS lane index varies by arrangement (1-bit for `.8b`, `.16b`; 2-bit for `.4h`, `.8h`; 3-bit for `.2s`, `.4s`). Out-of-range indices accepted and silently modulo-encoded.

## Root Cause

```rust
let lane = arr_n.chars().nth_back(1).unwrap().to_digit(10).unwrap() as u32;  // no validation
```

## Reproduction

**Input:** `ins v0.8b, v1.8b[4]`

**Expected:** `Err` — NEON lane index 4 out of range for .8b (valid: 0-1)

**Actual:** `Ok(Word(...))` — lane = 4 % 2 = 0, encoded as `[0]`

**Minimal failing input:** arr_d="8b", lane=2 (or 3, 8, 127, etc.)

## Impact

Lane indices silently modulo-encoded. User expects operation at specific lane but gets different encoding.

## Suggested Fix

Validate index against arrangement:

```rust
let max = match arr_d.as_str() {
    "8b" | "16b" => 1, "4h" | "8h" => 3, "2s" | "4s" => 7,
    _ => return Err(format!("unsupported arrangement: {}", arr_d)),
};
if lane > max {
    return Err(format!("lane index {} out of range for {}", lane, arr_d));
}
```

## Regression Property

Failing property: `neon_ins_lane_index_range_checked`

```rust
prop_assert!(encode_neon_ins(&[neon_reg(0, "8b"), neon_reg(1, "8b[4]")]).is_err());  // 8b: 0-1
prop_assert!(encode_neon_ins(&[neon_reg(0, "2s"), neon_reg(1, "2s[8]")]).is_err());  // 2s: 0-7
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/87