# Bug — `encode_orn` silently masks out-of-range shift amounts into imm6

**Function:** `encode_orn` — `src/backend/arm/assembler/encoder/data_processing.rs` (fn at line 910)
**PBT property:** `orn_xreg_shift_above_63_must_be_rejected` — **FAILS**
Also exposed by pre-existing `orn_negative_contracts` (clause 5b) — **FAILS** (W-register 32..63).

## Reproduction

```
cargo test --lib 'data_processing::tests::orn'
```

Minimal failing inputs (proptest-shrunk):
- `orn x0, x0, x0, lsl #64`  → accepted, encodes identically to `orn x0, x0, x0` (`lsl #0`)
- `orn w0, w0, w0, lsl #32`  → accepted (clause 5b), encodes as `lsl #0`

## Spec (ARMv8 ARM §C4.1.115)

The shifted-register ORN `imm6` field (bits 15:10) holds the shift amount:
- 64-bit (X) registers: valid range `0..=63`; `lsl #64` and above are UNDEFINED.
- 32-bit (W) registers: valid range `0..=31`; `32..=63` is UNPREDICTABLE.

GAS and llvm-mc reject these with `Error: immediate value out of range`.

## Root cause (`data_processing.rs:943`)

```rust
// ORN Rd, Rn, Rm [, shift #amount]: sf 01 01010 shift 1 Rm imm6 Rn Rd
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

The encoder unconditionally masks the shift amount with `& 0x3F` and never range-checks it,
so `lsl #64` aliases to `lsl #0`, `lsl #65` aliases to `lsl #1`, etc. — silently corrupting
the instruction rather than returning `Err`.

## Suggested fix

Range-check the shift against the width before encoding:
```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for orn", shift_amount));
}
```

## Impact

Any caller passing an out-of-range shift emits a different instruction than intended with no
diagnostic — a silent miscompilation of the source assembly.
