# Bug: `encode_logical` silently encodes UNDEFINED W-register shifts (32–63)

**Status:** CONFIRMED by property-based test (failing)
**Severity:** High (emits architecturally UNDEFINED instructions)
**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_logical`

## Summary

The scalar shifted-register branch of `encode_logical` masks the shift amount
to 6 bits with `(shift_amount & 0x3F)` and emits it directly into the `imm6`
field, **without validating it against the destination register width**. For
32-bit (`W`) registers, AArch64 constrains `imm6` to `[0, 31]`; a shift of
`32–63` is **UNDEFINED** (ARMv8 ARM, "Logical (shifted register)",
CONSTRAINED UNPREDICTABLE — may behave as `LSL #0`, corrupt the result, or
raise an UNDEFINED exception). GAS and LLVM-MC reject this input; this encoder
accepts it and produces a bogus word.

## Reproduction

Minimal failing input found by the property
`logical_w_reg_rejects_shift_above_31`:

```
rd = 0, rn = 0, rm = 0, amount = 32, sk = 0   (i.e. `and w0, w0, w0, lsl #32`)
```

`encode_logical(&[w0, w0, w0, Shift{lsl,32}], opc=0)` returns
`Ok(EncodeResult::Word(...))` instead of `Err`.

## Root cause (current code)

```rust
// AND/ORR/EOR Rd, Rn, Rm [, shift #amount]
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
    // ... maps kind -> 2-bit shift ...
    (st, *amount)
} else {
    (0, 0)
};

let word = ((sf << 31) | (opc << 29) | (0b01010 << 24) | (shift_type << 22))
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
//                       ^^^^^^^^^^^^^^^^^^  masks instead of rejecting
```

There is no width check. When `is_64 == false`, `imm6` must be `0..=31`.

## Suggested fix

Validate the shift amount against the register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-register logical op (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```

## Scope / related code (same bug class)

The identical `(shift_amount & 0x3F) << 10` masking-without-validation pattern
appears in several sibling encoders in the same file and should be checked
together:

- `encode_add_sub` (existing suite test #11 `w_reg_shifted_form_rejects_shift_above_31`
  asserts `Err` here too — it has the same latent failure).
- `encode_orn`, `encode_eon`, `encode_bics`, `encode_bic` (scalar reg form),
  `encode_mvn`, `encode_neg`, `encode_negs`.

Each should reject `shift_amount > (is_64 ? 63 : 31)`.

## Evidence

Property suite added in this file's `#[cfg(test)] mod tests`:

- `logical_register_form_field_placement` .......... PASS
- `logical_register_form_shift_mapping` ............ PASS
- `logical_sf_tracks_width` ........................ PASS
- `logical_w_reg_rejects_shift_above_31` ........... **FAIL** (this bug)
- `logical_immediate_form_roundtrips` .............. PASS
  (round-trips the encoder's `(N,immr,imms)` through an independent
   ARM-ARM `DecodeBitMasks` reference for every constructible bitmask)
- `logical_immediate_rejects_non_bitmask` .......... PASS

```
$ cargo test --lib data_processing::tests::logical
...
test result: FAILED. 5 passed; 1 failed
minimal failing input: rd = 0, rn = 0, rm = 0, amount = 32, sk = 0
```
