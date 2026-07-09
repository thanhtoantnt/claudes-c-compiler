# Bug Report: `encode_neon_movi` silently truncates out-of-range immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_movi`
**Severity:** Medium

## Summary

For byte/32-bit/16-bit element forms (`.8b`, `.16b`, `.2s`, `.4s`, `.4h`, `.8h`), `encode_neon_movi` masks immediate with `imm & 0xFF` and emits valid word for any input. ARMv8-A MOVI immediate is 8-bit (0..=255). Values outside range silently truncated.

## Root Cause

```rust
// .8b / .16b branch
let imm8 = imm as u32 & 0xFF;   // masks silently; no range check
```

## Reproduction

**Input:** `movi v0.8b, #256`

**Expected:** `Err` — MOVI immediate out of range (0-255): 256

**Actual:** `Ok(Word(251716608))` — 256 & 0xFF = 0, encodes as `movi v0.8b, #0`

**Other failing inputs:** `movi v0.8b, #-1` → encodes as `movi v0.8b, #255` (-1 wraps)

## Impact

Out-of-range immediates silently truncated. Inconsistent with `.2d` branch which validates strictly. LLVM-MC rejects these inputs.

## Suggested Fix

Validate range before masking:

```rust
if imm < 0 || imm > 255 {
    return Err(format!("MOVI immediate out of range (0-255): {}", imm));
}
let imm8 = imm as u32;
```

## Regression Property

Failing property: `out_of_range_immediate_must_be_rejected`

```rust
prop_assert!(encode_neon_movi(&[neon_reg(0, "8b"), 256]).is_err());    // overflow
prop_assert!(encode_neon_movi(&[neon_reg(0, "8b"), -1]).is_err());    // negative
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/183