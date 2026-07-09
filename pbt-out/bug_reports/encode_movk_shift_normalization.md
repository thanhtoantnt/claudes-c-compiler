# Bug Report: `encode_movk` silently normalizes invalid shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_movk`
**Severity:** High

## Summary

`encode_movk` computes MOVK halfword selector as `amount / 16` for `lsl` shifts. Does not require amount to be valid MOVK shift, so non-multiple-of-16 values silently floored to different shift.

## Root Cause

```rust
let hw = (*amount / 16) as u32;  // no validation of valid amounts
```

## Reproduction

**Input:** `movk x0, #1, lsl #1`

**Expected:** `Err` — movk invalid lsl shift: 1

**Actual:** `Ok(Word(...))` — hw = 1/16 = 0, encoded as no shift

**Other failing input:** `movk x0, #1, lsl #17` → encoded as `lsl #16`

## Impact

Invalid assembly accepted and encoded as different instruction than programmer wrote. Non-`lsl` shifts also treated as `hw = 0`.

## Suggested Fix

Reject non-`lsl` shifts and require exact valid amounts:

```rust
let hw = match (*amount, is_64) {
    (0, _) => 0,
    (16, _) => 1,
    (32, true) => 2,
    (48, true) => 3,
    _ => return Err(format!("movk invalid lsl shift: {}", amount)),
};
```

## Regression Property

Failing property: `movk_rejects_non_multiple_of_16_shift`

```rust
prop_assert!(encode_movk(&[xreg(0), imm(1), shift("lsl", 1)]).is_err());   // not multiple of 16
prop_assert!(encode_movk(&[xreg(0), imm(1), shift("lsl", 17)]).is_err());  // invalid for 64-bit
prop_assert!(encode_movk(&[wreg(0), imm(1), shift("lsl", 32)]).is_err());  // invalid for 32-bit
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/60