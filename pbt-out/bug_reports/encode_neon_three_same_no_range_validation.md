# Bug Report: `encode_neon_three_same` performs no range validation on immediate fields

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_three_same`
**Severity:** Medium

## Summary

`encode_neon_three_same` performs no range validation on the immediate fields for TBL/TBX. `imm3` is 3-bit (0-7), `imm4` is 4-bit (0-15). Out-of-range values accepted and silently modulo-encoded, producing potentially invalid encodings.

## Root Cause

```rust
let imm3 = imm3 & 0x7;  // no validation
let imm4 = imm4 & 0xF;  // no validation
```

## Reproduction

**Input:** `tbl v0.8b, {v0.8b, v0.8b, v0.8b, v0.8b, v0.8b}`

**Expected:** `Err` — NEON three-same immediate indices must be in range 0-7

**Actual:** `Ok(Word(...))` — imm3 = 8 & 0x7 = 0, encoded as index 0

## Impact

Immediate indices silently modulo-encoded. User expects operation at specific indices but gets different encoding.

## Suggested Fix

Validate against maximum values before masking:

```rust
if imm3 > 7 || imm4 > 15 {
    return Err(format!("NEON TBL/TBX indices out of range: imm3={}, imm4={}", imm3, imm4));
}
```

## Regression Property

Failing property: `neon_three_same_immediates_range_checked`

```rust
prop_assert!(encode_neon_three_same(&[neon_reg(0, "8b"), 
    vec![neon_reg(0, "8b"); neon_reg(0, "8b"); neon_reg(0, "8b"); 
    neon_reg(0, "8b"); neon_reg(0, "8b")], 8]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/180