# Bug Report: `encode_uxth` (64-bit form) emits MOV not UBFX

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxth`
**Severity:** Medium

## Summary

`encode_uxth` for 64-bit destination emits `0xD3403C00` which decodes as `ubfx x0, x0, #0, #16` — a `UBFX` instruction, not the intended `UXTH`. The encoder substitutes one instruction for another for architecturally-invalid 64-bit destination.

## Root Cause

```rust
let n = if is_64 { 1u32 } else { 0 };    // N=1 produces UBFX encoding, not UXTH
let word = ((sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22))
         | (15 << 10) | (rn << 5) | rd;
```

When `is_64 = true`, `N=1` yields `UBFX` encoding rather than `UXTH`.

## Reproduction

**Input:** `uxth x0, x0`

**Expected:** `Err` — UXTH is 32-bit-only

**Actual:** `Ok(Word(0xD3403C00))` → disassembles as `ubfx x0, x0, #0, #16`

## Impact

Silently emits wrong instruction. Users writing `uxth` get `ubfx` instead.

## Suggested Fix

Reject 64-bit destination before encoding:

```rust
if is_64 {
    return Err("uxth is 32-bit-only; use 'ubfx Xd, Xn, #0, #16' for 64-bit zero-extension".into());
}
```

## Regression Property

Failing property: `uxth_rejects_64bit_form_as_mov`

```rust
prop_assert!(encode_uxth(&[xreg(0), xreg(0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/127