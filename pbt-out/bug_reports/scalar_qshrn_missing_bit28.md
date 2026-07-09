# Bug: `encode_neon_scalar_qshrn` emits a clear bit 28 (reserved/UNDEFINED encoding)

## Location
`src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_scalar_qshrn`

## Summary
The scalar saturating-shift-right-narrow encoders (SQSHRN / SQRSHRN / UQSHRN / UQRSHRN
scalar) assemble the fixed opcode field `0b011110 << 23` for bits 28–23. The correct
AArch64 fixed bits for *Advanced SIMD scalar shift by immediate* (ARMv8 ARM C4.1.66) are
`0 1 U 1 1 1 1 1 0 ...`, i.e. **bit 28 = 1**. The encoder clears it, producing
**bit 28 = 0** in every emitted word. This makes every instruction UNDEFINED/reserved
on real hardware and disagrees with `llvm-mc`, `gas`, and `as`.

## Root cause
```rust
// current (wrong):
let word = (0b01 << 30) | (u_bit << 29) | (0b011110 << 23) | ...
//                              bit28=0 ^^^^

// should be:
let word = (0b01 << 30) | (u_bit << 29) | (0b111110 << 23) | ...
//                              bit28=1 ^^^^
```
`0b011110` (30) shifted into bits 28–23 gives `011110`; it must be `0b111110` (62) →
`111110`. The comment in the source even states the layout `01 U 11110 …` which omits
the mandatory fixed `1` at bit 28.

## Evidence (differential against `llvm-mc-18`, triple aarch64)

| mnemonic             | this encoder | llvm-mc      | XOR          |
|----------------------|--------------|--------------|--------------|
| `sqshrn  b0, h0, #1`  | `0x4F0F9400` | `0x5F0F9400` | `0x10000000` |
| `sqrshrn h0, s0, #16` | `0x4F109C00` | `0x5F109C00` | `0x10000000` |
| `uqshrn  s0, d0, #32` | `0x6F209400` | `0x7F209400` | `0x10000000` |
| `uqrshrn b5, h7, #4`  | `0x6F0C9CE5` | `0x7F0C9CE5` | `0x10000000` |

Every case differs by exactly bit 28. All other fields (Rd, Rn, immh:immb, U, opcode,
bit 10) are correct — confirmed by the five passing properties.

## PBT coverage
New module `scalar_qshrn_pbt_tests` (7 properties) in
`src/backend/arm/assembler/encoder/neon.rs`:

| Property | Result |
|---|---|
| `prop_matches_llvm_mc` (differential oracle vs `llvm-mc-18`) | **FAIL** — exposes bit 28 |
| `prop_fixed_high_bits` (bits 31-30=01, **bit 28=1**) | **FAIL** — exposes bit 28 |
| `prop_reg_fields_preserved` (Rd/Rn) | PASS |
| `prop_immh_immb_value` (immh:immb = 2·dest_bits − shift) | PASS |
| `prop_u_bit_and_opcode` (U bit, opcode, bit 10) | PASS |
| `prop_rejects_out_of_range_shift` (shift 0 / > dest bits → Err) | PASS |
| `prop_rejects_bad_dest_type` (d/q/v/x/w dest → Err) | PASS |

## Severity
High. Any emitted scalar SQSHRN-family instruction is malformed and would be rejected
by a real assembler / trapped as UNDEF on hardware. The encoder is otherwise internally
consistent, so the bad word can be silently written into an object.

## Fix
Change `0b011110 << 23` to `0b111110 << 23` (set bit 28). After this one-bit change all
seven properties pass.
