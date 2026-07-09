# Bug Report: `encode_negs` silently masks W-register shifts above 3

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

`encode_negs` masks shift amount with `& 0x3` without width-dependent validation. For 32-bit W-registers, amounts 4+ accepted and encoded as `amount % 4`, producing instruction with different shift.

## Root Cause

```rust
let st = *amount & 0x3;  // no width check before masking
```

## Reproduction

**Input:** `negs w0, w1, lsl #4`

**Expected:** `Err` — NEGS shift out of range: 4 (W-register valid: 0 only)

**Actual:** `Ok(Word(...))` — st = 4 & 0x3 = 0, encoded as `lsl #0`

**Minimal failing input:** is_64 = false, amount = 4 (or 5, 7, 8, 12, etc.)

## Impact

W-register shifts 4+ silently modulo 4. User expects operation at specific shift but gets different encoding.

## Suggested Fix

Validate against valid amounts before masking:

```rust
let valid = if is_64 { [0, 16, 32, 48] } else { [0] };
if !valid.contains(amount) {
    return Err(format!("negs shift out of range: {}", amount));
}
```

## Regression Property

Failing property: `negs_wreg_rejects_invalid_shift_amounts`

```rust
prop_assert!(encode_negs(&[wreg(0), wreg(1), shift("lsl", 4)], false).is_err());  // not 0
prop_assert!(encode_negs(&[wreg(0), wreg(1), shift("lsl", 16)], false).is_err()); // wraps to 0
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/82