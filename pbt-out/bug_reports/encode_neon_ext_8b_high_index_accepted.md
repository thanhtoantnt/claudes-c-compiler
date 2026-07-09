# Bug Report: `encode_neon_ext` accepts invalid high index for 8B arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ext`
**Severity:** Medium

## Summary

`encode_neon_ext` for `.8b` accepts high index values where only index 1 is valid. For `.8b` arrangements, NEON EXT has 1-bit index field (bit 11), valid values are 0 or 1. Indices 2+ accepted and encoded as `index % 2`.

## Root Cause

```rust
let index = arr_n.chars().skip(1).take(1).collect::<String>().parse::<u32>().ok()?;  // no validation
```

## Reproduction

**Input:** `ext v0.8b, v1.8b, #2`

**Expected:** `Err` — NEON EXT index for .8b must be 0 or 1

**Actual:** `Ok(Word(...))` — index = 2 % 2 = 0, encoded as `#0`

**Minimal failing input:** arr_d="8b", index = 2 (or 3, 5, 127, etc.)

## Impact

Invalid index values silently modulo-encoded. User expects operation at specific index but gets different encoding.

## Suggested Fix

Validate index against arrangement-specific max:

```rust
let max = match arr_d.as_str() {
    "8b" => 1, "16b" => 3, "4h" => 7, "8h" => 15, "2s" => 31, "4s" => 63,
    _ => return Err(format!("unsupported arrangement: {}", arr_d)),
};
if index > max {
    return Err(format!("EXT index {} out of range for {}", index, arr_d));
}
```

## Regression Property

Failing property: `neon_ext_index_range_checked`

```rust
prop_assert!(encode_neon_ext(&[neon_reg(0, "8b"), neon_reg(1, "8b"), 2]).is_err());
prop_assert!(encode_neon_ext(&[neon_reg(0, "16b"), neon_reg(1, "16b"), 4]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/84