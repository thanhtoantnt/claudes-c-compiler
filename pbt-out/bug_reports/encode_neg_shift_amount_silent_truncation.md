# Bug Report: `encode_neg` silently truncates shift amounts above 63

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** High

## Summary

`encode_neg` masks shift amount with `& 0x3F` without range check. For 64-bit registers, shift amounts 64+ accepted and encoded as `amount % 64`, producing different instruction.

## Root Cause

```rust
let imm6 = (*amount & 0x3F) as u32;  // no range check
```

## Reproduction

**Input:** `neg x0, x1, lsl #64`

**Expected:** `Err` — neg shift out of range: 64 (max is 63 for X-registers)

**Actual:** `Ok(Word(...))` — imm6 = 64 & 0x3F = 0, encoded as `lsl #0`

**Minimal failing input:** amount = 64 (or 127, 255, etc.)

## Impact

Silent truncation: shift amounts 64+ silently modulo 64. User expects operation at specific shift but gets different encoding.

## Suggested Fix

Validate range before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if *amount < 0 || *amount > max_shift {
    return Err(format!("neg shift out of range: {}", amount));
}
```

## Regression Property

Failing property: `neg_rejects_oversized_shift`

```rust
prop_assert!(encode_neg(&[xreg(0), xreg(1), shift("lsl", 64)]).is_err());
prop_assert!(encode_neg(&[xreg(0), xreg(1), shift("lsl", 127)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/76