# Bug Report: `encode_neon_ldnr` swaps R and S bits (LD2R/LD4R misencoded)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ldnr`
**Severity:** High

## Summary

`encode_neon_ldnr` stores the structure count (LD1R/LD2R/LD3R/LD4R) in the wrong bit field. Per ARMv8-A, the count belongs in **bit 21 (R field)** and **bit 12 (S)** is always 0 for this group. The implementation instead uses bit 12 for the count and never sets bit 21. For LD2R/LD4R, the encoder emits `bit12=1` (wrong) and `bit21=0` (wrong), swapping the R and S bits.

## Root Cause

```rust
// opcode: ld1r=110, ld2r=110(S=1), ld3r=111, ld4r=111(S=1)   <-- WRONG
let (opcode, s_bit) = match num_structs {
    1 => (0b110u32, 0u32),
    2 => (0b110, 1),   // should be: R=1, S=0
    3 => (0b111, 0),
    4 => (0b111, 1),   // should be: R=1, S=0
    ...
};
let word = ... | (s_bit << 12) | ...;  // bit 21 (R) never set
```

The R field (bit 21) should be 1 for LD2R/LD4R, not the S field (bit 12).

## Reproduction

**Input:** `ld2r {v0.16b, v1.16b}, [x1]`

**Expected:** `0x4D60C020` (llvm-mc-18 output, R=1, S=0)

**Actual:** `0x4D40D020` (XOR with correct = 0x20001000, bits 21 and 12 swapped)

**Minimal failing input:** num_structs=2 or num_structs=4

## Impact

LD2R/LD4R silently emit wrong machine code that decodes to a different instruction pattern. LD1R/LD3R happen to work (both R and S should be 0). XOR of actual-vs-correct is exactly `0x20001000` (bits 21 and 12).

## Suggested Fix

Encode count in R field (bit 21), force S to 0:

```rust
let (opcode, r) = match num_structs {
    1 => (0b110u32, 0u32),
    2 => (0b110, 1),
    3 => (0b111, 0),
    4 => (0b111, 1),
    _ => return Err(format!("unsupported: ld{}r", num_structs)),
};
...
let word = ... | (r << 21) | ...;  // no s_bit << 12
```

## Regression Property

Failing property: `prop_matches_arm_reference_encoding`

```rust
prop_assert_eq!(encode_neon_ldnr(&[vreg_arr(0, "16b"), vreg_arr(1, "16b"), reg_mem("x1")], 2, false),
               Ok(0x4D60C020));  // R=1, not S=1
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/202