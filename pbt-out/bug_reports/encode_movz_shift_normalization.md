# Bug Report: `encode_movz` silently normalizes invalid shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_movz`
**Severity:** High

## Summary

`encode_movz` computes halfword selector as `amount / 16` for `lsl` shifts. Does not require amount to be valid MOVZ shift, so non-multiple-of-16 values silently floored.

## Root Cause

```rust
let hw = (*amount / 16) as u32;  // no validation of valid amounts
```

## Reproduction

**Input:** `movz x0, #1, lsl #1`

**Expected:** `Err` — movz invalid lsl shift: 1

**Actual:** `Ok(Word(...))` — hw = 1/16 = 0, encoded as no shift

**Other failing input:** `movz x0, #1, lsl #17` → encoded as `lsl #16`

## Impact

Invalid assembly accepted and encoded as different instruction than programmer wrote.

## Suggested Fix

Require exact valid amounts:

```rust
let hw = match (*amount, is_64) {
    (0, _) => 0,
    (16, _) => 1,
    (32, true) => 2,
    (48, true) => 3,
    _ => return Err(format!("movz invalid lsl shift: {}", amount)),
};
```

## Regression Property

Failing property: `movz_rejects_non_multiple_of_16_shift`

```rust
prop_assert!(encode_movz(&[xreg(0), imm(1), shift("lsl", 1)]).is_err());   // not multiple of 16
prop_assert!(encode_movz(&[xreg(0), imm(1), shift("lsl", 17)]).is_err());  // invalid for 64-bit
prop_assert!(encode_movz(&[wreg(0), imm(1), shift("lsl", 32)]).is_err());  // invalid for 32-bit
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/229