# Bug — `encode_neon_aes` silently encodes unallocated opcodes

File: `src/backend/arm/assembler/encoder/neon.rs`
Function: `encode_neon_aes(operands: &[Operand], opcode: u32)`

## Summary
`encode_neon_aes` trusts the `opcode` parameter unconditionally and packs it
into the encoding with no range check. The AArch64 AES encoding space
allocates **only** opcodes `00100..00111` (AESE/AESD/AESMC/AESIMC). Any other
opcode value is **UNDEF** at runtime, yet this function returns `Ok` with a
valid-looking (but unallocated) word.

## Minimal failing input
```
encode_neon_aes(&[v0.16b, v1.16b], 8)   // opcode = 0b01000 (outside 00100..00111)
```

## Expected vs actual
- Expected: `Err(...)` — opcode 8 is unallocated.
- Actual: `Ok(Word(0x4E288800))`.

`0x4E288800` is not AESE/AESD/AESMC/AESIMC; it is an unallocated encoding that
would trap (UNDEF) on real hardware.

## How it was found
Negative-contract property `prop_unallocated_opcode_and_arrangement_must_error`
(proptest) in module `prop_encode_neon_aes_tests`. Shrunk to `bad_opcode = 8`.

## Impact
Latent today: the mnemonic dispatcher in `encoder/mod.rs` (lines 805–808) only
ever passes opcodes 4–7. But the `pub(crate)` signature provides no
defense-in-depth — any internal bug or future caller passing a bad opcode
emits a silently-unallocated instruction with no diagnostic. The four
positive properties (golden KAT, spec-oracle differential, layout, arity) all
pass, so the core packing is correct for valid opcodes.

## Suggested fix
Reject `opcode` outside `{0b00100, 0b00101, 0b00110, 0b00111}`:
```rust
if !matches!(opcode, 0b00100..=0b00111) {
    return Err(format!("encode_neon_aes: opcode {:#b} is not a valid AES opcode", opcode));
}
```

## Notes
- No AArch64-capable assembler was available (`/usr/bin/as` is x86-64
  binutils, rejects `aese`). Oracle = ARMv8-A ARM bit diagram + canonical
  golden words, hand-derived.
