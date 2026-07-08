# Bug Report — `encode_rbit` silently accepts mismatched scalar register widths

**File:** `src/backend/arm/assembler/encoder/bitfield.rs`
**Function:** `encode_rbit` (scalar branch)
**Severity:** Medium (silent mis-encoding of an UNALLOCATED instruction)
**Found by:** property-based tests in module
`prop_encode_rbit_scalar_width_coherence_tests`

## Summary

The scalar form `RBIT <Rd>, <Rn>` derives its `sf` (size) bit **only from the
destination register** and **discards the source register's width**. As a
result, mismatched-width operands such as `RBIT x0, w1` or `RBIT w0, x0` are
**silently accepted** and emitted as if the source shared the destination's
width — an instruction form the ARM ARM does not allocate. A width-coherent
assembler must reject these as an error.

## Root cause

```rust
pub(crate) fn encode_rbit(operands: &[Operand]) -> Result<EncodeResult, String> {
    // ... NEON vector branch ...
    // Scalar form: RBIT Rd, Rn
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- Rn width is read then THROWN AWAY
    let sf = sf_bit(is_64);                // <-- sf depends ONLY on Rd
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)`, but the `_` discards `Rn`'s `is_64`. The ARM
ARM "Data-processing (1 source)" group carries a **single** `sf` bit that
governs the width of **both** `Rd` and `Rn`, and lists only the two matched
forms:

```
RBIT <Wd>, <Wn>
RBIT <Xd>, <Xn>
```

There is no `<Xd>, <Wn>` or `<Wd>, <Xn>` form; those are UNALLOCATED.

## Reproduction

Property `prop_rejects_mismatched_scalar_widths` fails on minimal input:

```
rd = 0, rn = 0, rd_is_64 = false   →   operand pair: w0, x0
RBIT w0, x0  ⇒  Ok(Word(0x5AC00000))
```

`0x5AC00000` is the word for **`RBIT w0, w0`** — the `x0` source is silently
treated as `w0`. The reverse `RBIT x0, w0` ⇒ `Ok(0xDAC00000)` (the word for
`RBIT x0, x0`).

## Supporting evidence (diagnostic property passes)

`prop_source_width_silently_ignored` passes and pins the dead-parameter
behaviour:

```
RBIT x{A}, x{B}  ==  RBIT x{A}, w{B}     (bit-identical words)
```

This proves `Rn`'s scalar width has **zero** effect on the encoding, i.e. the
encoder is width-incoherent with respect to the source operand.

## Test results

```
prop_sf_tracks_matched_width .................. PASS   (baseline: sf tracks Rd width)
prop_matched_width_differential_only_sf ...... PASS   (X↔W changes only sf[31])
prop_source_width_silently_ignored ........... PASS   (defect evidence: Rn width dead)
prop_rejects_mismatched_scalar_widths ........ FAIL   (the finding)
```

## Suggested fix

In the scalar branch of `encode_rbit`, validate that `Rd` and `Rn` share the
same width and return `Err` otherwise, e.g.:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
if rd_is_64 != rn_is_64 {
    return Err(format!(
        "RBIT: register width mismatch (Rd is {}, Rn is {})",
        if rd_is_64 { "64-bit" } else { "32-bit" },
        if rn_is_64 { "64-bit" } else { "32-bit" },
    ));
}
let sf = sf_bit(rd_is_64);
```

## Related

The identical defect (single `sf` bit, `Rn` width discarded) is already
recorded for `encode_rev` in the existing `prop_encode_rev_*` tests in the
same file, and very likely affects the sibling scalar bit-reversal/rev
encoders (`encode_rev16`, `encode_rev32`) too.
