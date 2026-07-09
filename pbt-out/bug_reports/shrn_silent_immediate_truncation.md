# Bug: `encode_neon_shrn` silently truncates out-of-range shift immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs::encode_neon_shrn`
**Severity:** low–medium (input-validation gap; emits a valid-but-wrong encoding
with no diagnostic rather than crashing).

## Summary

`encode_neon_shrn` reads the shift immediate and casts it `i64 -> u32`
**before** its range check. An out-of-range immediate whose low 32 bits land
inside the legal `[1, half_bits]` range is therefore silently accepted and
encoded as that truncated shift, instead of being rejected.

## Root cause

```rust
let shift = get_imm(operands, 2)? as u32;          // truncation here
...
let half_bits = element_bits / 2;
if shift == 0 || shift > half_bits { return Err(...); }   // checked on the truncated value
```

`get_imm` returns `i64`. The `as u32` wrap loses the high 32 bits *before* the
bound check, defeating it for any value congruent mod 2³² to an in-range shift.

## Reproduction

Witness test (kept `#[ignore]` so `cargo test` stays green):
`backend::arm::assembler::encoder::neon_shrn_pbt::prop_shrn_truncates_huge_immediate`

```
encode_neon_shrn([v0.8b, v1.8h, Imm(0x1_0000_0001)], 0b100001, false)
  => Ok(Word(0x0F0F8420))     // identical to `shrn v0.8b, v1.8h, #1`
```

`0x1_0000_0001` (4 294 967 297) wraps to `1` as `u32`, is accepted, and emits the
`#1` encoding. The ARMv8 ARM restricts the shift to a small integer in `[1, 32]`
(per source arrangement), so this value must be rejected. A value one past the
true max (e.g. `9` for `.8h`) shifted across the 2³² boundary so it lands back
in range (`0x1_0000_0002` → `2`) is accepted likewise.

## Suggested fix

Validate the full `i64` before narrowing:

```rust
let shift_i = get_imm(operands, 2)?;
let half_bits = element_bits / 2;
if shift_i <= 0 || shift_i as u64 > half_bits as u64 {
    return Err(format!("shrn: shift {} out of range (1..={})", shift_i, half_bits));
}
let shift = shift_i as u32;
```

This same `get_imm(..)? as u32`-then-check pattern is shared by several sibling
encoders in `neon.rs` (e.g. `encode_neon_sqshrun`, `encode_neon_qshrn`,
`encode_neon_shift_right`, `encode_neon_shll`) and they likely exhibit the same
gap.

## Coverage of correct behavior

All other property-based tests for `encode_neon_shrn` **pass**: the bit layout
for every valid input matches an independent reference encoder; every field
(Rd, Rn, Q, immh:immb, opcode) decomposes correctly; no `immh == 0000`
(unallocated) encoding is ever produced; and genuine out-of-range cases
(`shift == 0`, `shift == half_bits + 1`, unsupported arrangement, missing /
non-immediate operand) are all correctly rejected. The encoding logic is
otherwise correct.
