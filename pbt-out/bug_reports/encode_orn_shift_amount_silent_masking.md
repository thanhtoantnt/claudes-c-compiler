# Bug Report: `encode_orn` silently truncates shift amounts above 31

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`
**Severity:** High

## Summary

`encode_orn` masks shift amount with `& 0x3F` without width-dependent validation. For 32-bit W-register forms, shifts 32+ accepted and encoded as `amount % 64`, producing instruction with different shift.

## Root Cause

```rust
let imm6 = shift & 0x3F;  // no width check
```

## Reproduction

**Input:** `orn w0, w1, w2, lsl #32`

**Expected:** `Err` — ORN shift out of range: 32 (W-register max is 31)

**Actual:** `Ok(Word(...))` — imm6 = 32 % 64 = 32

**Minimal failing input:** is_64 = false, shift = 32 (or 64, 96, 127)

## Impact

W-register shifts 32+ accepted, encoded with potentially wrong semantics.

## Suggested Fix

Validate width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift < 0 || shift > max_shift {
    return Err(format!("orn shift out of range: {}", shift));
}
```

## Regression Property

Failing property: `orn_rejects_oversized_shift`

```rust
prop_assert!(encode_orn(&[wreg(0), wreg(1), wreg(2)], shift("lsl", 32)]).is_err());
prop_assert!(encode_orn(&[xreg(0), xreg(1), xreg(2)], shift("lsl", 64)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/92