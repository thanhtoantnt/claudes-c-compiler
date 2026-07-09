# Bug Report: `encode_uxth` silently accepts 32-bit destination

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxth`
**Severity:** Medium

## Summary

`encode_uxth` accepts 32-bit `W` destination register but ARMv8-A UXTH is 32-bit-only. Attempting to use `uxth w0, x0` (mixed width) silently encodes as 32-bit with wrong source register.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // source width discarded
```

## Reproduction

**Input:** `uxth w0, x0`

**Expected:** `Err` — UXTH requires same-width registers

**Actual:** `Ok(Word(...))` — x0 encoded as if it were w0

## Impact

Mixed-width forms accepted. Source register width information discarded.

## Suggested Fix

Validate source/destination width match:

```rust
let (rd, rd_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
if rd_64 || rn_64 != rd_64 {
    return Err("UXTH requires 32-bit (W) registers".into());
}
```

## Regression Property

Failing property: `uxth_rejects_mixed_width`

```rust
prop_assert!(encode_uxth(&[wreg(0), xreg(0)]).is_err());  // mixed width
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/128