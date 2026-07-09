# Bug: `encode_neon_scalar_three_same` silently accepts out-of-range `u_bit`/`size`/`opcode`

## Summary

`encode_neon_scalar_three_same` (in
`src/backend/arm/assembler/encoder/neon.rs`) OR-shifts its `u_bit`, `size`, and
`opcode` parameters directly into the 32-bit instruction word **without any
range check or masking**. Per the ARMv8-A ARM, these are fixed-width fields with
no wrapping semantics, so an out-of-range value must be rejected with `Err`.
Instead the function returns `Ok(EncodeResult::Word(..))` and silently corrupts
the adjacent architecturally-constant bits.

## Affected code

```rust
pub(crate) fn encode_neon_scalar_three_same(
    operands: &[Operand], u_bit: u32, opcode: u32, size: u32,
) -> Result<EncodeResult, String> {
    // ...
    let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Encoding layout (ARM DDI 0487, "Scalar Three Same")

```
  31 30 29 28-24 23-22 21 20-16 15-11 10 9-5 4-0
   0  1  U  11110  size  1   Rm   opcode 1  Rn  Rd
```

| Field  | Bits    | Width | Valid range |
|--------|---------|-------|-------------|
| scalar | 31-30   | 2     | `01` (fixed)|
| U      | 29      | 1     | 0..=1       |
| const  | 28-24   | 5     | `11110`     |
| size   | 23-22   | 2     | 0..=3       |
| const  | 21      | 1     | `1`         |
| Rm     | 20-16   | 5     | 0..=31      |
| opcode | 15-11   | 5     | 0..=31      |
| const  | 10      | 1     | `1`         |
| Rn     | 9-5     | 5     | 0..=31      |
| Rd     | 4-0     | 5     | 0..=31      |

None of these fields are defined to wrap; an out-of-range value produces an
**unallocated** encoding.

## Impact / repro

The two production call sites (`add`/`sub` D-register forms in `mod.rs`) always
pass in-range values, so currently-emitted `add`/`sub` are correct. However any
future caller, refactor, or fuzzer that supplies a bad `u_bit`/`size`/`opcode`
will get a plausible-looking `Ok` word with corrupted constant bits instead of
an error, defeating the encoder's own `Result<_, String>` contract.

Minimal failing case (from the `#[ignore]`d property
`rejects_out_of_range_params`):

```text
encode_neon_scalar_three_same(&[d0, d1, d2], u_bit=2, opcode=0b10000, size=0b11)
  → Ok(Word(0x5EE28420))          // bit 31 flipped: scalar marker 01 → 11 (expected Err)
encode_neon_scalar_three_same(&[d0, d1, d2], u_bit=4, opcode=0b10000, size=0b11)
  → Ok(Word(0xDEE28420))          // bits 31-30: 01 → 11
encode_neon_scalar_three_same(&[d0, d1, d2], u_bit=0, opcode=0b10000, size=0b100)
  → Ok(Word(0x5FE28420))          // bit 24 flipped: 11110 → 11111
encode_neon_scalar_three_same(&[d0, d1, d2], u_bit=0, opcode=0b100000, size=0b11)
  → Ok(Word(0x5EE30420))          // bit 16 flipped: Rm corrupted (2 → 3)
```

Register numbers *are* correctly bounded (0..=31) by `parse_reg_num`, so
`d32`+ is rejected — only the three parameter fields are unguarded.

## Suggested fix

Validate the parameter widths before assembly, e.g.:

```rust
if u_bit > 1     { return Err(format!("scalar three-same: u_bit {u_bit} out of range (0..=1)")); }
if size > 0b11   { return Err(format!("scalar three-same: size {size} out of range (0..=3)")); }
if opcode > 0b1F { return Err(format!("scalar three-same: opcode {opcode} out of range (0..=31)")); }
```

## Test coverage

`src/backend/arm/assembler/encoder/neon_scalar_three_same_pbt.rs` adds:

- `matches_golden_table` — absolute hex oracle (incl. `add d0,d1,d2=0x5EE28420`,
  `sub d0,d1,d2=0x7EE28420`).
- `matches_reference_encoder` — differential oracle vs. an independent
  field-by-field assembler (passes).
- `fields_round_trip` — Rd/Rn/Rm/U/size/opcode placement (passes).
- `fixed_bits_are_constant` — bits 31-30=`01`, 28-24=`11110`, 21=`1`, 10=`1`
  (passes).
- `rejects_malformed_operands` — arity / operand kind / register >= 32 (passes).
- `rejects_out_of_range_params` — **the finding**, `#[ignore]`d until fixed;
  run with `cargo test --lib neon_scalar_three_same -- --ignored`.
