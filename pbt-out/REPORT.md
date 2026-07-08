# REPORT — `encode_madd`

## Summary

Generated a property-based test suite for `encode_madd`
(`src/backend/arm/assembler/encoder/data_processing.rs:598`), the AArch64
`MADD <Rd>, <Rn>, <Rm>, <Ra>` encoder. Five properties were written; **four pass,
one fails**. The failing property is a legitimate spec-violation witness:
`encode_madd` does not validate that all four operands share the same register
width, so mixed W/X operands are silently encoded as the `Rd` width.

(Pre-existing context: the `data_processing` test binary already had ~46 failing
tests from prior campaigns — unrelated to this work; results below are isolated
to `data_processing::madd_props`.)

## Modules Tested

| Module | Target | Oracle | Properties | Result |
|---|---|---|---|---|
| `data_processing::madd_props` | `encode_madd` (data_processing.rs:598) | reference constant + field placement + differential (vs `encode_msub`) + negative contract | 5 | 4 pass / 1 fail |

## Bugs Found

**`encode_madd` silently accepts mixed-width register operands** —
full report: `pbt-out/bug_reports/encode_madd_mixed_width_operands.md`.

`sf` is derived only from `Rd` (operand 0); the widths of `Rn`/`Rm`/`Ra` are
bound to `_` and discarded, so `madd x0, w0, x0, x0` encodes as a 64-bit `MADD`
instead of erroring. The ARMv8 ARM requires all four MADD operands to share one
width.

Witness (failing, shrunk PBT property):
- **property:** `madd_rejects_mixed_width_operands` (mod `madd_props`,
  data_processing.rs:6013)
- **reproduce:** `cargo test --lib data_processing::madd_props::madd_rejects_mixed_width_operands`
- **Falsifiable / minimal failing input:** `n = 0` (successes before failure: 0)
- **counterexample:** `ops = [Reg("x0"), Reg("w0"), Reg("x0"), Reg("x0")]`
  → `madd x0, w0, x0, x0`
- **actual:** `Ok(EncodeResult::Word(…))` with `sf = 1`
- **expected:** `Err`

The same defect class affects the adjacent `encode_msub` (line 608) and
`encode_mul` (line 584); each has / needs its own report
(`encode_msub_mixed_width_operands.md` already exists in `pbt-out/bug_reports/`).

## Design Caveats

- **Register 31 encodes as XZR/WZR and is valid for MADD.** The generated
  register range (`0u32..=31`) includes 31; P1/P2 verify it encodes correctly.
  This is spec-correct (ARMv8 ARM, Data-processing (3 source): the zero register
  is permitted in every MADD operand), not a defect — hence not filed.
  *Doc evidence:* existing differential/characterization convention in
  `data_processing.rs` (`encode_madd`/`encode_msub` both place Ra=31 for the
  XZR alias, e.g. `encode_mul` line 594 `0b11111 << 10`).
- **No other evidence-backed intentional-behavior caveats.** The mixed-width
  acceptance was the only non-spec-correct behavior observed; per reporting
  rules it is reclassified as a bug (see ## Bugs Found), not a caveat.

## Test Files Created

| File | Change | Type |
|---|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` | appended `#[cfg(test)] mod madd_props` (~135 lines, 5 `proptest!` properties) | inline PBT module |

No new top-level test files; the suite was appended as a sibling
`#[cfg(test)] mod madd_props` to match the file's existing convention
(`mod smull_props`, `mod smaddl_props`, `mod mvn_props`, …).

## Output Directories

| Path | Contents |
|---|---|
| `pbt-out/bug_reports/encode_madd_mixed_width_operands.md` | Bug report for the mixed-width finding (failing-property witness) |
| `pbt-out/REPORT.md` | This report |
