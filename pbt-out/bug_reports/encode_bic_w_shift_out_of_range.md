# Bug Report — `encode_bic`: W-register shifted form silently accepts shift ≥ 32

**Function:** `encode_bic` in
`src/backend/arm/assembler/encoder/data_processing.rs`

**Severity:** Medium (emits an UNDEFINED AArch64 encoding instead of an
assembler error)

## Summary

In the scalar shifted-register branch, the shift amount is masked into the
6-bit `imm6` field with `& 0x3F` for *both* 32-bit (W) and 64-bit (X) forms,
with no width-based range check:

```rust
// BIC is AND with N=1 (bit 21): sf opc=00(bits29:30) 01010 shift 1 Rm imm6 Rn Rd
let word = (sf << 31) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
         | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

For a 64-bit register, `imm6 ∈ [0, 63]` is valid, so masking is correct. For a
32-bit register (`sf == 0`), the ARMv8 ARM restricts `imm6` to `[0, 31]`; any
larger value is UNDEFINED. GAS and LLVM-MC reject such encodings (e.g.
`bic w0, w0, w0, lsl #32`). This implementation instead silently encodes
`imm6 = 32`, producing a word that is architecturally UNDEFINED.

This is the same defect class already documented for `encode_add_sub`
(`w_reg_shifted_form_rejects_shift_above_31`) and `encode_mvn`
(`mvn_w_reg_rejects_shift_above_31`) in this module.

## Reproduction

Property `bic_w_register_rejects_shift_above_31` (negative contract) fails
with minimized input:

```
rd = 0, rn = 0, rm = 0, amount = 32, sk = 0   // bic w0, w0, w0, lsl #32
```

The call returns `Ok(EncodeResult::Word(..))`; the test asserts `is_err()`.

Run:

```
cargo test --lib backend::arm::assembler::encoder::data_processing::tests::bic
```

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

All other `encode_bic` branches are correct for the tested inputs:

- **Scalar register form** (`bic Xd, Xn, Xm`): `opc=00`, fixed op `01010`,
  `N=1` (bit 21, distinguishing BIC from AND), zero shift, `Rm`/`Rn`/`Rd`
  placement, and `sf` width tracking — for both W and X registers.
- **Scalar shifted-register form** (`lsl`/`lsr`/`asr`/`ror`): correct 2-bit
  shift-type field mapping, `imm6` placed verbatim for the 0..63 X-register
  range, `N=1` preserved regardless of shift.
- **Immediate form** (`bic Xd, Xn, #imm`): bit-identical to `and Xd, Xn, #(~imm)`
  for every immediate (differential oracle); encodability of `#imm` as BIC is
  exactly encodability of `#(~imm)` as AND, with opc=00, fixed `100100`, and
  correct `Rn`/`Rd` placement.
- **NEON vector form** (`bic Vd.T, Vn.T, Vm.T`, `T ∈ {8b, 16b}`): `Q` bit
  selects 128-bit vs 64-bit, bits 31/29 are 0, fixed `01110` at bits 28:24,
  size/op `01` at bits 23:22, `N=1`, fixed `000111` at bits 15:10, and
  `Rm`/`Rn`/`Rd` placement.

## Regression property

Failing property: `bic_w_register_rejects_shift_above_31`

```rust
prop_assert!(encode_bic(&[wreg(rd), wreg(rn), wreg(rm)], "lsl", 32).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/10
