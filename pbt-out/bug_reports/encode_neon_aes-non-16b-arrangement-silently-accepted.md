# Bug — `encode_neon_aes` silently accepts non-`.16B` arrangements

File: `src/backend/arm/assembler/encoder/neon.rs`
Function: `encode_neon_aes(operands: &[Operand], opcode: u32)`

## Summary
`encode_neon_aes` discards the register arrangement returned by
`get_neon_reg` (`let (rd, _) = get_neon_reg(...)`). The ARM ARM requires the
`.16B` arrangement **exclusively** for all four AES instructions (AESE/AESD/
AESMC/AESIMC); any other arrangement (`.8b`, `.4s`, `.2d`, …) is invalid
syntax. This encoder accepts any arrangement and produces the identical bytes
as the `.16B` form.

## Minimal failing input
```
encode_neon_aes(&[v0.8b, v1.8b], 0b00100)   // AESE with .8b (invalid)
```

## Expected vs actual
- Expected: `Err(...)` — `aese v0.8b, v1.8b` is invalid; only `.16B` allowed.
- Actual: `Ok(Word(0x4E284800))` — byte-identical to the valid
  `aese v0.16b, v1.16b`.

## How it was found
Negative-contract property `prop_unallocated_opcode_and_arrangement_must_error`
(proptest) in module `prop_encode_neon_aes_tests`, branch `bad_arr = "8b"`.
(The opcode branch of the same property surfaces a separate bug, filed
independently.)

## Impact
User-reachable. A typo such as `aese v0.8b, v1.8b` (or `.4s`, `.2d`, …)
assembles silently to a valid-looking word instead of reporting the syntax
error that a real assembler (gas/llvm-mc) would reject. Because the AES
encoding carries no arrangement/Q bits, the malformed source is
indistinguishable from correct `.16B` source in the emitted object — the error
is invisible downstream.

## Suggested fix
Validate the arrangement of both operands after parsing:
```rust
for i in 0..2 {
    let (_, arr) = get_neon_reg(operands, i)?;
    if arr != "16b" {
        return Err(format!("AES instructions require .16B arrangement, got .{}", arr));
    }
}
```

## Notes
- No AArch64-capable assembler was available (`/usr/bin/as` is x86-64
  binutils, rejects `aese`). Oracle = ARMv8-A ARM bit diagram + canonical
  golden words, hand-derived.
