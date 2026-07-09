# Bug Report: `encode_neon_scalar_two_misc` out-of-range u_bit/opcode silently corrupts encoding

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_scalar_two_misc`
**Severity:** Medium

## Summary

`encode_neon_scalar_two_misc` OR-shifts `u_bit` and `opcode` into the instruction word without range validation. The ARMv8-A scalar two-register miscellaneous encoding has: `01 U 11110 size 10000 opcode 10 Rn Rd`, where `U` is 1-bit (bit 29) and `opcode` is 5-bit (bits 16:12). Out-of-range values silently overflow into adjacent constant fields, producing malformed, UNDEFINED instruction words with no error.

## Root Cause

```rust
let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22)
    | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
```

No bounds check before assembly:
- `u_bit = 2` flips bit 30, destroying the `01` scalar marker (bits [31:30] become `11`)
- `opcode = 32` flips bit 17, breaking the fixed `10000` pattern at [21:17]

## Reproduction

**Input:** `encode_neon_scalar_two_misc(&[d0, d1], u_bit=2, opcode=0b00111)`

**Expected:** `Err` — u_bit must be 0 or 1

**Actual:** `Ok(Word(0x7EE07820))` — `01` scalar marker became `11`, wrong encoding

**Minimal failing input:** u_bit=2 or opcode=32

## Impact

Silent mis-encoding for out-of-range values. Latent — current callers pass fixed in-range constants (`sqabs`, `sqneg`). Any future caller or fuzz input with wider values emits garbage silently.

## Suggested Fix

Validate field widths before assembly:

```rust
if u_bit > 1 {
    return Err(format!("scalar two-misc: u_bit out of range (0..=1), got {}", u_bit));
}
if opcode > 0x1F {
    return Err(format!("scalar two-misc: opcode out of range (0..=31), got {}", opcode));
}
```

## Regression Property

Failing property: `rejects_out_of_range_params`

```rust
prop_assert!(encode_neon_scalar_two_misc(&[dreg(0), dreg(1)], 2, 0b00111).is_err());   // u_bit overflow
prop_assert!(encode_neon_scalar_two_misc(&[dreg(0), dreg(1)], 0, 32).is_err());       // opcode overflow
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/176