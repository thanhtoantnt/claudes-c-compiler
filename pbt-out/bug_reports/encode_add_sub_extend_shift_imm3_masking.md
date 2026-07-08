# Bug — `encode_add_sub` extended-register shift silently truncated (`& 0x7`)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` →
`encode_add_sub`, the `Operand::Extend { .. }` branch (covers UXTX and all
extends).

## Summary

The optional additional shift for the ADD/SUB extended-register form
(`imm3`, bits 12:10) is masked with `& 0x7` instead of being range-checked.
The ARMv8 ARM restricts this shift to **0..=4**; values 5–7 are
UNDEFINED, yet the encoder silently accepts them (and any amount ≥8 whose
masked value lands in 0–7).

## Relevant code

```rust
if let Some(Operand::Extend { kind, amount }) = operands.get(3) {
    let option = match kind.as_str() { /* ... */ };
    let imm3 = *amount & 0x7;                       // <-- silently truncates
    let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24))
             | (1 << 21) | (rm << 16) | (option << 13) | (imm3 << 10)
             | (rn << 5) | rd;
    return Ok(EncodeResult::Word(word));
}
```

## Differential check (clang `--target=aarch64`)

```
$ echo 'add x0, x1, x2, uxtx #5' | clang --target=aarch64-linux-gnu -c -o /dev/null -
error: expected 'sxtx' 'uxtx' or 'lsl' with optional integer in range [0, 4]
```

## Property test (EXPECTED FAIL)

`add_uxtx_shift_above_4_must_be_rejected` — for `amount in 5u32..=7u32`,
`encode_add_sub(...).is_err()`. Minimal failing input: `amount = 5`.

## Fix

Validate `amount ∈ 0..=4` and return `Err` otherwise, instead of
`let imm3 = *amount & 0x7;`.

## Positive coverage that still passes (core UXTX encoding is correct)

- `add_uxtx_reference_encoding` — full word == `0x8B206000 | rm<<16 | rn<<5 | rd`
  (clang: `add x0,x1,x2,uxtx` → `0x8b226020`).
- `add_uxtx_shift_round_trips` — imm3 round-trips for valid 0..=4.
- `add_uxtx_op_and_flags_propagate` — op/S bits track inputs, UXTX option preserved.

## Reproduce

```
cargo test --lib data_processing::tests::add_uxtx_shift_above_4_must_be_rejected
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/124
