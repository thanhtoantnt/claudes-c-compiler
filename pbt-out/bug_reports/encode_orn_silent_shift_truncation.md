# BUG: `encode_orn` silently encodes out-of-range shift amounts (UNPREDICTABLE per ARMv8)

## Target
`src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn` (scalar, shifted-register form), line ~946:

```rust
// ORN Rd, Rn, Rm [, shift #amount]: sf 01 01010 shift 1 Rm imm6 Rn Rd
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

## Finding (functional)
For the **32-bit (W-register, `sf=0`)** operand form, the imm6 shift amount must be
in the range **0..=31**. ARMv8 ARM §C4.1.4 / §C4.1.115 (ORN, shifted register) state
that for `sf=0` a shift amount of 32..=63 is **UNPREDICTABLE** (the literal value
`#32..#63` is reserved). The encoder masks the value with `& 0x3F` and **silently
accepts** any `u32`, including 32..=63, emitting a malformed instruction word
instead of returning `Err`.

The same defect exists in the sibling encoders `encode_eon`, `encode_bic`,
`encode_bics`, `encode_logical` (ORR/AND/EOR shifted-reg path), `encode_mvn`,
`encode_neg`, `encode_negs` — they all apply `& 0x3F` with no `sf=0` range check.
(Existing PBT for `mvn`/`bic`/`bics` already documents this failing assertion.)

## Reproduction
Property `orn_negative_contracts` (part b) fails:

```
minimal failing input: n = 0, rd = 0, rn = 0, rm = 0, amount = 32, sk = 0
```
i.e. `ORN W0, W0, W0, LSL #32` is accepted by `encode_orn` and encoded as
`0x0A200000` (imm6 = `100000` = 32), rather than being rejected.

## Impact
- Assembler accepts syntactically invalid shift amounts and produces an
  UNPREDICTABLE encoding; downstream consumers (objdump, emulator, real CPU)
  will disagree on its meaning.
- Silent acceptance masks source-level bugs in hand-written asm.
- No error signal for the caller to handle.

## Suggested fix
Validate the shift amount against the operand width before encoding, e.g.:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "orn: shift amount {} out of range for {}-bit register (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```
Apply the same guard to `encode_eon`, `encode_bic`, `encode_bics`,
`encode_logical` (shifted-reg branch), `encode_mvn`, `encode_neg`, `encode_negs`.

## Properties written (this run)
In the `encode_orn` test block of `data_processing.rs::tests`:

| # | Property | Result |
|---|----------|--------|
| 1 | `orn_register_form_field_placement` — opc=01, op5=01010, N=1, regs placed | PASS |
| 2 | `orn_register_form_shift_mapping` — 4 shift kinds → 2-bit field; imm6 0..63 verbatim (X regs) | PASS |
| 3 | `orn_differs_from_orr_only_in_n_bit` — differential vs `encode_logical(opc=01)`: XOR == `1<<21` | PASS |
| 4 | `orn_neon_vector_form_fields` — bit31=0, Q tracks 8b/16b, op5=01110, size/op=11, fixed6=000111 | PASS |
| 5 | `orn_negative_contracts` — (a) <3 operands → Err [PASS]; (b) W-reg shift 32..63 → Err | **FAIL (part b)** |
