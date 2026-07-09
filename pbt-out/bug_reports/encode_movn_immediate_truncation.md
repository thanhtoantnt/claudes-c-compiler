# Bug Report: `encode_movn` silently truncates out-of-range immediate magnitude

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_movn`
**Severity:** High

## Summary

`encode_movn` masks immediate with `& 0xFFFF` without validation. Out-of-range immediates accepted and silently encoded as different value.

## Root Cause

```rust
let imm16 = (imm as u32) & 0xFFFF;  // no range check
```

## Reproduction

**Input:** `movn x0, #65536` (0x10000)

**Expected:** `Err` — movn immediate out of range: 65536

**Actual:** `Ok(Word(...))` — imm16 = 0x10000 & 0xFFFF = 0x0000, identical to `movn x0, #0x0`

**Minimal failing input:** rd = 0, imm = 65536

## Impact

Silent miscompilation: constants assembled through MOVN can lose upper bits with no diagnostic.

## Suggested Fix

Validate immediate before encoding:

```rust
if imm < 0 || imm > 0xFFFF {
    return Err(format!("movn immediate out of range: {}", imm));
}
```

## Regression Property

Failing property: `movn_rejects_out_of_range_immediate`

```rust
prop_assert!(encode_movn(&[xreg(0), imm(65536)]).is_err());    // overflow
prop_assert!(encode_movn(&[xreg(0), imm(-1)]).is_err());       // negative
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/61