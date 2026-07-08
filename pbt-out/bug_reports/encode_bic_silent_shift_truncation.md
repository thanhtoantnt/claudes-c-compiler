# Bug Report: `encode_bic` silently truncates out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_bic`
(shifted-register form, scalar)
**Severity:** Medium (silent mis-compilation of an invalid mnemonic)
**Status:** Reproduced by property `bic_props::bic_rejects_oversized_shift`
(minimal failing input: `bic x0, x1, x2, lsl #64`).

## Summary

`encode_bic`'s shifted-register branch writes the shift amount into the 6-bit
`imm6` field with a bare `& 0x3F` mask and **no range validation**:

```rust
let word = (sf << 31) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

Per the ARMv8 ARM (§C4.1.4, *Logical (shifted register)*), the `imm6` field is
only 6 bits, so a shift amount **> 63 is unrepresentable** and the mnemonic must
be rejected. Instead the encoder masks the value and emits a valid-looking — but
semantically wrong — instruction.

## Concrete counterexample

`bic x0, x1, x2, lsl #64` returns `Ok(Word)` with `imm6 = 0`, i.e. it encodes
identically to `bic x0, x1, x2` (no shift). The user asked to clear bit 64 of
`x2` shifted left by 64; they silently get an unshifted `BIC`.

```
lsl #0   -> imm6 = 0   (correct)
lsl #64  -> imm6 = 0   (WRONG — silently collides with lsl #0, should be Err)
lsl #65  -> imm6 = 1   (silently becomes lsl #1)
lsl #128 -> imm6 = 0   (silently becomes lsl #0)
```

A conforming assembler rejects these:
* `aarch64-linux-gnu-as`: `Error: immediate value out of range at operand 3`
* `llvm-mc`: `error: expected compatible register or immediate`

## Secondary case (32-bit registers)

For `W`-register operands (`sf = 0`) the ARMv8 encoding additionally requires
`imm6 < 32` (bit 5 of `imm6` must be 0; `imm6 >= 32` is reserved / UNDEFINED).
`encode_bic` applies the same `& 0x3F` mask regardless of width, so e.g.
`bic w0, w1, w2, lsl #32` is also accepted silently and encodes a reserved
encoding.

## Expected vs. actual

| Mnemonic                     | Expected | Actual                                   |
|------------------------------|----------|------------------------------------------|
| `bic Xd,Xn,Xm, lsl #64`      | `Err`    | `Ok` → encoded as `lsl #0`               |
| `bic Xd,Xn,Xm, lsl #65`      | `Err`    | `Ok` → encoded as `lsl #1`               |
| `bic Wd,Wn,Wm, lsl #32`      | `Err`    | `Ok` → reserved encoding (imm6 bit 5 = 1)|

## Suggested fix

Validate the shift amount against the operand width before encoding, mirroring
what `encode_shift`/`encode_logical`-style helpers do elsewhere. In the
shifted-register branch of `encode_bic`:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "bic shift amount {} out of range [0, {}]", shift_amount, max_shift
    ));
}
```

(For `ror` on 64-bit registers the architecturally-valid range is `1..=63`;
`0` is CONSTRAINED UNPREDICTABLE — at minimum reject `> 63` as above.)

## Regression property

Failing property: `bic_rejects_oversized_shift`

```rust
prop_assert!(encode_bic(&[xreg(rd), xreg(rn), xreg(rm)], "lsl", 64).is_err());
```

## Test evidence

`bic_props::bic_rejects_oversized_shift` (in `data_processing.rs`) asserts
`encode_bic(.., lsl #amount)` for `amount ∈ 64..=4095` must return `Err`. It
currently fails; once the fix lands it will pass, at which point the four
companion properties (`bic_register_form_fields`,
`bic_register_form_shift_fields`, `bic_sf_tracks_register_width`,
`bic_immediate_is_and_of_inverted`) remain green.
