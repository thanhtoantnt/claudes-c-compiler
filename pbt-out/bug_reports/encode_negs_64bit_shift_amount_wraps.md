# Bug Report: `encode_negs` shift amounts wrap around for X-registers

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

`encode_negs` masks shift amount with `& 0x3` without range validation. For 64-bit X-registers, amounts 64+ wrap (e.g., 64 → 0), producing instruction with different shift than intended.

## Root Cause

```rust
let st = *amount & 0x3;  // no range check before masking
```

## Reproduction

**Input:** `negs x0, x1, lsl #4`

**Expected:** `Err` — NEGS shift out of range: 4 (valid: 0, 16, 32, 48 for X-registers)

**Actual:** `Ok(Word(...))` — st = 4 & 0x3 = 0, encoded as `lsl #0`

**Minimal failing input:** is_64 = true, amount = 4 (or 68, 132, etc.)

## Impact

Shift amounts silently wrapped modulo 4. User expects operation at specific shift but gets different encoding.

## Suggested Fix

Validate against valid amounts before masking:

```rust
let valid = if is_64 { [0, 16, 32, 48] } else { [0, 0, 0, 0] };
if !valid.contains(amount) {
    return Err(format!("negs shift out of range: {}", amount));
}
```

## Regression Property

Failing property: `negs_xreg_rejects_invalid_shift_amounts`

```rust
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("lsl", 4)], true).is_err());  // not 0/16/32/48
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("lsl", 68)], true).is_err()); // wraps to 4
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/80