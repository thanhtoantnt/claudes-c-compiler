# Bug Report: `encode_ldtr_sized` performs no offset range validation

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldtr_sized`
**Severity:** High

## Summary

`encode_ldtr_sized` masks offset with `& 0x1FF` without range validation. For 64-bit registers, the imm9 field must be in `[-256, 255]`. Values outside this range accepted and silently modulo-encoded.

## Root Cause

```rust
let offset = (*offset & 0x1FF) as i64;  // no range check
```

## Reproduction

**Input:** `ldtr w0, [x1, #256]`

**Expected:** `Err` — LDTR offset out of range: 256 (valid: -256 to 255)

**Actual:** `Ok(Word(...))` — offset = 256 & 0x1FF = 0, encoded as no offset

**Minimal failing input:** is_64 = true, offset = 256 (or 512, 1024, etc.)

## Impact

Silent truncation: values outside [-256, 255] encoded as different values. User expects operation at specific offset but gets different encoding.

## Suggested Fix

Validate range before masking:

```rust
let max = if is_64 { 255 } else { 127 };
if offset < -max || offset > max {
    return Err(format!("LDTR offset out of range: {} (valid: [-{}, {}])", offset, -max, max));
}
```

## Regression Property

Failing property: `ldtr_rejects_out_of_range_offset`

```rust
prop_assert!(encode_ldtr_sized(&[xreg(0), mem_offset(xreg(1), 256)]).is_err());
prop_assert!(encode_ldtr_sized(&[wreg(0), mem_offset(wreg(1), 128)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/119