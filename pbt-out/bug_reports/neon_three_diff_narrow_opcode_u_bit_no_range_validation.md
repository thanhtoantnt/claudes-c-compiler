# Bug Report: `encode_neon_three_diff_narrow` out-of-range `opcode`/`u_bit` silently corrupts encoding

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_three_diff_narrow`
**Severity:** Medium

## Summary

`encode_neon_three_diff_narrow` OR-shifts `u_bit` and `opcode` into the instruction word without range validation. `u_bit` is a 1-bit field (bit 29) and `opcode` is a 4-bit field (bits 15-12). Out-of-range values overflow into neighbouring fields, producing silently-corrupted instruction words instead of `Err`.

## Root Cause

```rust
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
    | (rm << 16) | (opcode << 12) | (rn << 5) | rd;
```

- `opcode >= 0x10` overflows upward into the Rm field (bits 20-16)
- `u_bit >= 2` overflows into the Q field (bit 30)

Both produce a silently-different, architecturally unrelated instruction word.

## Reproduction

**Input:** `encode_neon_three_diff_narrow(&ops, u_bit=2, opcode=0b0100, false)`

**Expected:** `Err` — u_bit must be 0 or 1

**Actual:** `Ok(Word(...))` — u_bit=2 leaks into Q bit, turning ADDHN into ADDHN2

**Minimal failing input:** opcode_oob = 16, u_bit_oob = 2

## Impact

Latent defect — current callers use fixed in-range constants. A future instruction or dynamic caller would emit garbage silently. Same defect as sibling `encode_neon_three_diff`.

## Suggested Fix

Validate field widths before assembly:

```rust
if u_bit > 1 {
    return Err(format!("three-diff narrow: u_bit must be 0 or 1, got {}", u_bit));
}
if opcode > 0xF {
    return Err(format!("three-diff narrow: opcode must be 4-bit (0..=0xF), got {}", opcode));
}
```

## Regression Property

Failing property: `narrow_rejects_out_of_range_opcode_and_u_bit`

```rust
prop_assert!(encode_neon_three_diff_narrow(&ops, 2, 0b0100, false).is_err());   // u_bit overflow
prop_assert!(encode_neon_three_diff_narrow(&ops, 0, 0x10, false).is_err());     // opcode overflow
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/36
