# Bug Report: `encode_mvn` silently truncates out-of-range shift amounts

## Target
`src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_mvn`
(scalar path; vector path delegates to `encode_neon_not` and is unaffected).

## Summary
`MVN Xd, Xm [, <shift> #amount]` masks the shift amount with `& 0x3F` before
placing it in the imm6 field and never validates the range. A shift amount
greater than 63 overflows the 6-bit imm6 field and is **silently wrapped**
rather than rejected. Reference assemblers reject the same input.

## Reproduction (PBT)
Property `mvn_props::out_of_range_shift_amount_is_rejected` fails with the
minimal failing input `rd = 0, rm = 0, amount = 64`.

The encoding built for `mvn x0, x0, lsl #64` is **identical** to the encoding
for `mvn x0, x0` (no shift):

```
imm6 = 64 & 0x3F == 0
```

Likewise `mvn x0, x0, lsl #100` encodes imm6 = `100 & 0x3F == 36` — i.e. it
is emitted as if the programmer had written `lsl #36`. The intended operand is
corrupted with no diagnostic.

## Reference behavior (oracle)
Both GNU `as` and LLVM `llvm-mc` reject an out-of-range logical shift:

```
$ echo "mvn x0, x0, lsl #64" | llvm-mc -triple=aarch64 -
error: immediate value out of range

$ echo "mvn x0, x0, lsl #64" | as --64 -o /dev/null -
Error: immediate value out of range
```

The AArch64 ARM places the shift in a 6-bit imm6 field, so `#64` cannot be
represented and is architecturally illegal.

## Root cause
```rust
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
```
The `& 0x3F` mask silently truncates; there is no bounds check returning `Err`.

## Scope
The same `& 0x3F`-with-no-validation pattern recurs across the logical/shift
family, so the bug is broader than `encode_mvn`:
- `encode_orn`, `encode_eon`, `encode_bics`, `encode_bic` (scalar)
- `encode_logical`
- `encode_neg`, `encode_negs` (these alias into shifted-register ADD/SUB)

## Suggested fix
Validate the shift amount against the register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-bit register", shift_amount,
        if is_64 { 64 } else { 32 }
    ));
}
```

Note the secondary issue this also catches: for 32-bit (W) registers, imm6
values 32..=63 are architecturally UNPREDICTABLE; GAS rejects them too, but the
current code encodes them (e.g. `mvn w0, w0, lsl #40` succeeds and emits imm6=40).

## Properties written (module `mvn_props`, in `data_processing.rs`)
| # | Property | Result |
|---|----------|--------|
| 1 | `mvn_default_form_field_placement` | PASS |
| 2 | `rn_field_always_xzr` | PASS |
| 3 | `mvn_shift_type_and_amount_fields` | PASS |
| 4 | `sf_bit_tracks_register_width` | PASS |
| 5 | `mvn_equals_orn_with_xzr_rn` (differential vs `encode_orn`) | PASS |
| 6 | `out_of_range_shift_amount_is_rejected` (negative contract) | **FAIL** |
