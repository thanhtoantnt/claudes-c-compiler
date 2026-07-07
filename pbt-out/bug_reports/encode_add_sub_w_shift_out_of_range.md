# Bug Report — `encode_add_sub`: W-register shifted form silently accepts shift ≥ 32

**Function:** `encode_add_sub` in
`src/backend/arm/assembler/encoder/data_processing.rs`

**Severity:** Medium (emits an UNDEFINED AArch64 encoding instead of an
assembler error)

## Summary

In the shifted-register branch, the shift amount is masked into the 6-bit
`imm6` field with `& 0x3F` for *both* 32-bit (W) and 64-bit (X) forms:

```rust
let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24) | (shift_type << 22))
         | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

For a 64-bit register, `imm6 ∈ [0, 63]` is valid, so masking is correct.
For a 32-bit register (`sf == 0`), the ARMv8 ARM restricts `imm6` to
`[0, 31]`; any larger value is UNDEFINED. GAS and LLVM-MC reject such
encodings (e.g. `add w0, w0, w0, lsl #32`). This implementation instead
silently encodes `imm6 = 32`, producing a word that is architecturally
UNDEFINED.

## Reproduction

Property `w_reg_shifted_form_rejects_shift_above_31` (negative contract) fails
with minimized input:

```
rd = 0, rn = 0, rm = 0, amount = 32, sk = 0   // add w0, w0, w0, lsl #32
```

The call returns `Ok(EncodeResult::Word(..))`; the test asserts `is_err()`.

## Expected behavior

For 32-bit shifted-register operands, `shift_amount` outside `[0, 31]` must be
rejected with an `Err`, matching GAS/LLVM-MC.

## Suggested fix

Validate the shift amount against register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-bit register (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```

## What passes (verified by the new suite)

All other `encode_add_sub` branches are correct for the tested inputs:

- Immediate form (unshifted, auto/`lsl #12` shifted, negative-imm op flip,
  out-of-range → `Err`, `sf` width).
- Shifted-register form: `lsl`/`lsr`/`asr` shift-type field, `imm6` placement,
  `S` bit, register placement (64-bit).
- Extended-register form: all 8 extend kinds (`uxtb`…`sxtx`) map to the correct
  `option` field with bit 21 set and `imm3 == 0`.
- SP operand (rd or rn = `sp`) correctly routes to the extended-register form
  with `option = UXTX` so register 31 reads as SP, not XZR.
- Relocation modifiers: `:lo12:` → `AddAbsLo12`, `:tprel_lo12_nc:` →
  `TlsLeAddTprelLo12`, `:tprel_hi12:` → `TlsLeAddTprelHi12` with `sh = 1`;
  `imm12` left zero for the linker in all three.
