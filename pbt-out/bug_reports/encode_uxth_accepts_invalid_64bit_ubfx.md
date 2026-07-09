# Bug Report: `encode_uxth` accepts architecturally-invalid 64-bit form

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxth`
**Severity:** High

## Summary

UXTH is a 32-bit-only alias of `UBFM`. The 64-bit form `uxth <Xd>, <Xn>` has no valid encoding. This encoder silently accepts it and produces a word that decodes as a different instruction (`UBFX`).

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;  // width from dest only
let sf = sf_bit(is_64);
let n = if is_64 { 1u32 } else { 0 };    // N=1 when 64-bit → UBFX, not UXTH
let word = ((sf << 31) | ...) | (15 << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `uxth x0, x0`

**Expected:** `Err` — uxth requires 32-bit (W) destination register

**Actual:** `Ok(Word(0xD3403C00))` — emits `ubfx x0, x0, #0, #16` (wrong instruction)

## Impact

Silently produces different instruction than written. llvm-mc-18 rejects `uxth x0, x0` with "invalid operand for instruction".

## Suggested Fix

Reject 64-bit destination:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
if is_64 {
    return Err("uxth requires a 32-bit (W) destination register".to_string());
}
```

## Regression Property

Failing property: `uxth_rejects_64bit_destination_form`

```rust
prop_assert!(encode_uxth(&[xreg(0), xreg(0)]).is_err());  // X destination invalid
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/126