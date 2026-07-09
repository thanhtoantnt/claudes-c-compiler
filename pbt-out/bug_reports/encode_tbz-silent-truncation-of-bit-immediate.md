# Bug Report: `encode_tbz-silent-truncation-of-bit-immediate`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_tbz`
**Severity:** High

## Summary

`encode_tbz` masks bit immediate with `& 0x1F` without validation. ARMv8-A allows only bit positions 0-31. Values outside this range silently modulo-encoded, potentially wrapping to target the wrong bit.

## Root Cause

```rust
let bit = bit & 0x1F;  // no range check
```

## Reproduction

**Input:** `tbz x0, #35`

**Expected:** `Err` — TBZ bit out of range (valid: 0-31)

**Actual:** `Ok(Word(...))` — bit = 35 & 0x1F = 3, encoded as `tbz x0, #3`

**Minimal failing input:** bit = 32 (or 33-63, or negative values)

## Impact

Silent truncation/wrapping. User expects operation at specific bit position but gets different encoding. Same defect as `tbnz`.

## Suggested Fix

Validate range before masking:

```rust
if bit < 0 || bit > 31 {
    return Err(format!("TBZ bit out of range (valid: 0-31): {}", bit));
}
```

## Regression Property

Failing property: `tbz_rejects_out_of_range_bit`

```rust
prop_assert!(encode_tbz(&[xreg(0), 35]).is_err());
prop_assert!(encode_tbz(&[wreg(0), -1]).is_err());    // negative
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/163