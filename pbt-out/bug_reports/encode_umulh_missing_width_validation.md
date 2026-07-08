# Bug Report — `encode_umulh` silently accepts 32-bit (W) registers

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_umulh`

## Summary

`UMULH` is a **64-bit-only** AArch64 instruction (ARMv8 ARM, C4.1.68 / "Data-processing
(2 source)", opcode `111110`). There is no 32-bit form. A reference assembler therefore
rejects any `W` (32-bit) operand. `encode_umulh`, however, hard-codes `sf = 1` and
**discards** the width returned by `get_reg`, so it silently accepts `W` operands and
emits an `Ok(EncodeResult::Word(...))` instead of `Err`.

## Current (buggy) code

```rust
pub(crate) fn encode_umulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // width discarded
    let (rn, _) = get_reg(operands, 1)?;   // width discarded
    let (rm, _) = get_reg(operands, 2)?;   // width discarded
    // UMULH: 1 00 11011 1 10 Rm 0 11111 Rn Rd
    let word = (1u32 << 31) | (0b0011011110 << 21) | (rm << 16) | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The `let (rd, _) = ...` pattern throws away the `is_64: bool` flag for every operand, and
no check ever validates that all three are 64-bit.

## Evidence

Differential check against an independent reference assembler
(`clang --target=aarch64-linux-gnu`):

```
$ echo '.text
umulh w0, w1, w2' | clang --target=aarch64-linux-gnu -c -x assembler -o /tmp/o.o -
wreg.s:2:7: error: invalid operand for instruction
umulh w0, w1, w2
      ^
```

The encoder, by contrast, returns `Ok(Word(0x9BC07C00))` for `umulh w0, w1, w2`.

Failing property-based test (`umulh_props::umulh_rejects_wrong_width_operands`):

```
minimal failing input: n = 0
  umulh w0, w1, w2  ->  Ok(Word(2613083136))   // 0x9BC07C00 — should be Err
```

## Impact

- Assembles an instruction that no conforming AArch64 assembler would accept,
  producing a 64-bit `UMULH` encoding from operands the user wrote as 32-bit `W`.
  The user's intent is lost silently with no diagnostic.
- Inconsistent with the rest of the crate: sibling encoders already enforce the
  analogous 64-bit-only contract (e.g. the existing `smull`/`smulh` property suites
  assert `W → Err`).

## Note on what is *correct*

The encoding itself is **correct** for valid inputs. The differential oracle
(`umulh_matches_llvm_reference`, base constant `0x9BC07C00` derived from clang) passes
for the full register range `x0..x31`, and field placement (`sf=1`, Rm bits 20:16,
Rn bits 9:5, Rd bits 4:0) is verified. Only the missing width validation is wrong.

## Suggested fix

Reject any non-64-bit operand:

```rust
pub(crate) fn encode_umulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, rd64) = get_reg(operands, 0)?;
    let (rn, rn64) = get_reg(operands, 1)?;
    let (rm, rm64) = get_reg(operands, 2)?;
    if !(rd64 && rn64 && rm64) {
        return Err("umulh requires 64-bit (X) registers".to_string());
    }
    let word = (1u32 << 31) | (0b0011011110 << 21) | (rm << 16) | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Test suite

Added `mod umulh_props` with 5 property-based tests in
`src/backend/arm/assembler/encoder/data_processing.rs`:

| # | Property | Result |
|---|----------|--------|
| P1 | `umulh_matches_llvm_reference` — differential oracle vs clang (`0x9BC07C00` base) | PASS |
| P2 | `umulh_field_placement` — `sf=1`, Rm/Rn/Rd field placement | PASS |
| P3 | `umulh_rejects_too_few_operands` — negative contract | PASS |
| P4 | `umulh_rejects_non_register_operands` — negative contract | PASS |
| P5 | `umulh_rejects_wrong_width_operands` — spec: `W` → `Err` | **FAIL (this bug)** |
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/107
