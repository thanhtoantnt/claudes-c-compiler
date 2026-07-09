# Bug Report — `encode_shift`: no range validation on immediate shift amount; arithmetic overflow panic

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_shift`
**File:** `src/backend/arm/assembler/encoder/data_processing_shift_pbt.rs`
**Witness:** `shift_rejects_invalid_immediate` (marked `#[ignore]` — default `cargo test` stays green)

## Summary

The immediate form of `encode_shift` (`LSL/LSR/ASR/ROR Rd, Rn, #imm`) performs
**no range validation** on the shift amount `imm`. Out-of-range or negative
immediates are either silently mis-encoded or — for `LSL` — cause an unsigned
subtraction overflow that **panics in debug builds**.

## Architecture constraint (ARMv8 ARM)

The immediate shift amounts have fixed, width-dependent valid ranges:

| mnemonic | valid `imm` for 32-bit | valid `imm` for 64-bit |
|----------|------------------------|------------------------|
| `LSL`    | `0..=31`               | `0..=63`               |
| `LSR`    | `0..=31`               | `0..=63`               |
| `ASR`    | `0..=31`               | `0..=63`               |
| `ROR`    | `1..=31`               | `1..=63`               |

Anything outside these ranges is UNALLOCATED and an assembler must reject it.

## Root cause

```rust
let imm = *imm as u32;                 // (1) negative i64 wraps to a huge u32
let width = if is_64 { 64 } else { 32 };
...
// LSL branch:
let immr = (width - imm) % width;      // (2) underflows when imm > width
let imms = width - 1 - imm;            // (3) underflows when imm >= width
```

* (1) `Operand::Imm` is `i64`; casting a negative value with `as u32` silently
  wraps to a large positive, so negative shifts are never rejected.
* (2)/(3) For `imm >= width`, `width - imm` and `width - 1 - imm` underflow.
  In debug builds this panics (`attempt to subtract with overflow`); in release
  it wraps and emits a word with corrupted `immr`/`imms` fields.
* The `LSR`/`ASR`/`ROR` branches (`immr = imm`, `imm << 10`) don't underflow but
  silently truncate / shift the value into neighbouring fields for
  out-of-range `imm`, again without error.

## Reproduction

Minimal input surfaced by the property:

```
shift_type = 0  (LSL),  is_64 = false,  imm = 32   // i.e.  lsl w0, w1, #32
```

```
thread '...' panicked at src/backend/arm/assembler/encoder/data_processing.rs:810:28:
attempt to subtract with overflow
   --> let immr = (width - imm) % width;     // line 810
   --> let imms = width - 1 - imm;           // line 811
```

Run it explicitly:

```
cargo test --lib data_processing_shift_pbt::shift_rejects_invalid_immediate -- --ignored
```

## Suggested fix

Validate `imm` against the architectural range before encoding, e.g. for the
immediate branch:

```rust
let imm = *imm;
if imm < 0 || imm as u32 >= width {
    return Err(format!("shift immediate {} out of range for {}-bit register", imm, width));
}
let imm = imm as u32;
```

(plus an additional `imm != 0` check for `ROR`, whose immediate form is
`1..=width-1`).

## Tests

* `shift_register_reference_encoding`, `shift_register_field_placement`,
  `shift_immediate_field_oracle` — pass (register form and in-range immediate
  form are encoded correctly).
* `shift_rejects_missing_shift_operand` — passes (negative contract).
* `shift_rejects_invalid_immediate` — `#[ignore]`d witness, fails as described.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/301
