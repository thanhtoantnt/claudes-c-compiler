# Bug Report: `encode_fp_1src`: `opcode` never range-checked, silently corrupts encoding

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fp_1src`
**Severity:** High

## Summary

`opcode` OR'd into 6-bit field `[20:15]` with no validation. Overflow bits corrupt adjacent fields: bit 6→21 (fixed 1), bit 7→22 (ftype low), bit 8→23 (ftype high). Silent wrong-code emission on any out-of-range `opcode`.

## Root Cause

```rust
let word = (0b00011110u32 << 24) | (ftype << 22) | (1 << 21)
    | (opcode << 15) | (0b10000 << 10) | (rn << 5) | rd;
// No opcode range check
```

## Reproduction

**Input:** `encode_fp_1src(&[sreg(0), sreg(1)], 128)`

**Expected:** `Err` — fp 1-source opcode out of range: 128

**Actual:** `Ok(Word(0x1E604000))` — ftype corrupted to 01 (single→double)

**Minimal failing input:** opcode = 64 (aliases to 0 via bit 21), opcode = 128 (corrupts ftype)

## Impact

Latent today (callers pass only valid constants). Any future caller or typo emits wrong machine code with no `Err`. Opcode=64 silently aliases to 0; opcode=128 rewrites single-precision dest as double.

## Suggested Fix

Reject out-of-range opcodes:

```rust
if opcode > 0x3F {
    return Err(format!("fp 1-source opcode out of range: {}", opcode));
}
```

## Regression Property

Failing property: `prop_fp_1src_rejects_out_of_range_opcode`

```rust
prop_assert!(encode_fp_1src(&[sreg(0), sreg(1)], 64).is_err());   // overflow
prop_assert!(encode_fp_1src(&[sreg(0), sreg(1)], 128).is_err());  // corrupts ftype
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/129