# Bug Report: `encode_sbfiz` accepts out-of-range `immr`/`imms` values

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_sbfiz`
**Severity:** High

## Summary

`encode_sbfiz` masks `immr` and `imms` with `& 0x3F` without range validation. ARMv8-A restricts both fields to 0-31. Out-of-range values accepted and silently masked, producing invalid encodings.

## Root Cause

```rust
let immr = immr & 0x3F;  // no validation
let imms = imms & 0x3F;  // no validation
```

## Reproduction

**Input:** `sbfiz w0, w0, #0, #35`

**Expected:** `Err` — SBFIZ imms field out of range (valid: 0-31)

**Actual:** `Ok(Word(...))` — imms = 35 & 0x3F = 3

**Minimal failing input:** immr = 32 (or imms = 32, or both = 33, 127, 255, etc.)

## Impact

Out-of-range values silently truncated. User expects operation at specific parameters but gets different encoding. Reference assemblers reject these inputs.

## Suggested Fix

Validate ranges before masking:

```rust
if immr < 0 || immr > 31 {
    return Err(format!("SBFIZ immr out of range: {} (valid: 0-31)", immr));
}
if imms < 0 || imms > 31 {
    return Err(format!("SBFIZ imms out of range: {} (valid: 0-31)", imms));
}
```

## Regression Property

Failing property: `sbfiz_rejects_out_of_range_operands`

```rust
prop_assert!(encode_sbfiz(&[wreg(0), wreg(0), imm(0), imm(35)]).is_err());
prop_assert!(encode_sbfiz(&[xreg(0), xreg(0), imm(0), imm(32)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/214