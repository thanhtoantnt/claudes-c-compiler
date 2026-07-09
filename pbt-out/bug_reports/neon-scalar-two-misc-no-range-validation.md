# Bug: `encode_neon_scalar_two_misc` does not validate out-of-range `u_bit` / `opcode`

## Target
`encode_neon_scalar_two_misc` in
`src/backend/arm/assembler/encoder/neon.rs` (line ~1819).

## Summary
`u_bit` and `opcode` are fixed-width fields in the AArch64 "Scalar
two-register miscellaneous" encoding (`01 U 11110 size 10000 opcode 10 Rn Rd`):

- `U`      — 1 bit at position **[29]**
- `opcode` — 5 bits at positions **[16:12]**

The ARM ARM defines no wrapping semantics for these fields, so any value
exceeding the field width is unrepresentable and **MUST** be rejected with
`Err`. The implementation instead OR-shifts the raw value into the word with
**no masking or range check**:

```rust
let word = (0b01 << 30) | (u_bit << 29) | (0b11110 << 24) | (size << 22)
    | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
```

Out-of-range arguments therefore silently overflow into adjacent constant
fields and emit a malformed, architecturally-UNDEFINED instruction word with
no error.

## Impact
- `u_bit = 2` flips **bit 30**, destroying the `01` scalar marker (bits
  [31:30] become `11` → a different encoding class).
- `opcode = 32` (0x20) flips **bit 17**, breaking the fixed `10000` pattern at
  [21:17]; larger `opcode` values corrupt bits [16:12] and bleed into [21]/[10].
- The downstream assembler writer has no signal that the emitted word is
  garbage; the only current "validation" is the caller passing the right
  literal, which is enforced nowhere.

Latent foot-gun: the only call sites today
(`sqabs` → `u=0,opc=0b00111`, `sqneg` → `u=0,opc=0b01000` in `mod.rs`) happen
to pass in-range values, so it is not currently triggered in production. Any
future caller (or fuzz/test input) that supplies a wider value gets a silently
wrong encoding.

## Reproduction
Property `rejects_out_of_range_params` in
`src/backend/arm/assembler/encoder/neon_scalar_two_misc_pbt.rs`
(marked `#[ignore]` because the impl violates the contract):

```
cargo test --lib backend::arm::assembler::encoder::neon_scalar_two_misc_pbt::rejects_out_of_range_params -- --ignored
```

Minimal failing inputs:
- `encode_neon_scalar_two_misc(&[d0, d1], u_bit=2, opcode=0b00111)`
  → returns `Ok(Word(0x7EE07820))`; the `01` scalar marker became `11`.
- `encode_neon_scalar_two_misc(&[d0, d1], u_bit=0, opcode=32)`
  → returns `Ok(Word(...))` with bit 17 corrupted; expected `Err`.

## Suggested fix
Mask-and-assert before assembling the word:

```rust
if u_bit > 1 {
    return Err(format!("scalar two-misc: u_bit {} out of range (0..=1)", u_bit));
}
if opcode > 0x1F {
    return Err(format!("scalar two-misc: opcode {} out of range (0..=31)", opcode));
}
```

## Severity
Low (not currently reachable from valid assembly) / Medium (silent
mis-encoding on any wider input). Same class of bug as the sibling
`encode_neon_scalar_three_same`.
