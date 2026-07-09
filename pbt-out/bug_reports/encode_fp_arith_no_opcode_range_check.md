# Bug Report: `encode_fp_arith` does not validate `opcode` range — silent overflow into Rm field

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fp_arith`
**Severity:** Low (latent — callers pass valid opcodes, but no guard exists)

## Summary

`opcode` OR'd unconditionally at `opcode << 12` with **no mask and no range check**. Opcode field is 4 bits `[15:12]`. Any `opcode >= 16` overflows into adjacent 5-bit Rm field `[20:16]`, producing silently-corrupted word with different Rm and opcode.

## Root Cause

```rust
let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21)
         | (rm << 16) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
// No opcode range check
```

## Reproduction

**Input:** `encode_fp_arith(&[dreg(0), dreg(0), dreg(0)], 16)`

**Expected:** `Err` — fp_arith opcode 16 exceeds 4-bit field

**Actual:** `Ok(Word(...))` — bit 16 adds 1 to encoded Rm value, wrong instruction

**Minimal failing input:** opcode = 16 (0b10000), or any opcode >= 16

## Impact

Future caller passing wide opcode or mistaken constant gets silently-malformed instruction instead of error. 32-bit opcode can clobber `ftype`, bit 21, and fixed bits.

## Suggested Fix

Reject out-of-range opcodes:

```rust
if opcode > 0b1111 {
    return Err(format!("fp_arith opcode {} exceeds 4-bit field", opcode));
}
```

## Regression Property

Failing property: `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`

```rust
prop_assert!(encode_fp_arith(&[dreg(0), dreg(0), dreg(0)], 16).is_err());    // overflow
prop_assert!(encode_fp_arith(&[dreg(0), dreg(0), dreg(0)], 32).is_err());    // massive overflow
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/148