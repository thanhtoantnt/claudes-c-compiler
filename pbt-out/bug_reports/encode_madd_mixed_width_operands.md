# Bug Report — `encode_madd` silently accepts mixed-width register operands

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_madd` (line 598)
**Severity:** correctness / silent mis-assembly

## Summary

AArch64 `MADD <Rd>, <Rn>, <Rm>, <Ra>` requires **all four** operands to share the
same register width (all `W` or all `X`). `encode_madd` derives the `sf`
(operand-size) bit **only** from `Rd` (operand 0) and discards the widths of
`Rn`, `Rm`, `Ra`. Consequently `madd x0, w0, x0, x0` is silently encoded as a
64-bit `MADD` instead of being rejected.

## Witness (failing PBT property)

```
property : madd_rejects_mixed_width_operands
           (mod madd_props, data_processing.rs:6013)
status   : FAILED  (4 passed; 1 failed)
reproduce: cargo test --lib data_processing::madd_props::madd_rejects_mixed_width_operands
Falsifiable / minimal failing input: n = 0   (successes before failure: 0)
counterexample:
    ops = [Reg("x0"), Reg("w0"), Reg("x0"), Reg("x0")]   // madd x0, w0, x0, x0
    result = Ok(EncodeResult::Word(0x9B000000 | ...))     // sf=1, accepted
expected: Err  (32-bit Rn in a 64-bit MADD is invalid)
```

The property asserts `encode_madd(&[x0, w0, x0, x0]).is_err()`; proptest shrank
to `n = 0`.

## Root cause

```rust
pub(crate) fn encode_madd(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width taken ONLY from Rd
    let (rn, _) = get_reg(operands, 1)?;       // width discarded
    let (rm, _) = get_reg(operands, 2)?;       // width discarded
    let (ra, _) = get_reg(operands, 3)?;       // width discarded
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (0b0011011000 << 21) | (rm << 16)) | (ra << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`is_64` is read from operand 0; the `bool` widths returned for operands 1–3 are
bound to `_` and never compared against `is_64`.

## Impact

- **Silent mis-assembly.** A typo or IR bug producing `madd x0, w0, x0, x0`
  emits a 64-bit instruction (full 64-bit write to `Rd`) instead of an error,
  masking the originating bug.
- **Inconsistency.** Sibling encoders in this file validate width consistency
  via negative-contract properties that fail on the same defect:
  `adc_rejects_mixed_width_operands` (data_processing.rs:2884),
  `sbc_rejects_mixed_width_operands` (3002),
  `orn_rejects_mixed_register_widths` (3441),
  `eon_rejects_mixed_register_widths` (5422),
  `smull_rejects_wrong_width_destination`. `encode_madd` is the outlier.
- The same defect class exists in the adjacent `encode_msub` (line 608) and
  `encode_mul` (line 584, lowers to `MADD …, XZR`); each needs its own report.

## Suggested fix

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, n64) = get_reg(operands, 1)?;
let (rm, m64) = get_reg(operands, 2)?;
let (ra, a64) = get_reg(operands, 3)?;
if is_64 != n64 || is_64 != m64 || is_64 != a64 {
    return Err("madd operands must all be the same register width".into());
}
```

## Related existing evidence

A pre-existing characterization test `madd_silently_accepts_mixed_width_operands`
(data_processing.rs:3788, comment 3783-3787) already documents the acceptance
without asserting it is correct. This report upgrades it to a confirmed
spec-violation with a failing property.
