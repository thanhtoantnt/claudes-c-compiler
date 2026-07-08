# REPORT — `encode_sxtb`

## Summary

Generated a property-based test suite for `encode_sxtb`
(`src/backend/arm/assembler/encoder/data_processing.rs:872`), the AArch64
`SXTB <Rd>, <Rn>` encoder (alias of `SBFM <Rd>, <Rn>, #0, #7`). Six properties
were written; **five pass, one fails**. The failing property is a legitimate
spec-violation witness: `encode_sxtb` does not validate that the source and
destination registers share the same width, so a 32-bit destination paired with a
64-bit source (`sxtb w0, x0`) is silently emitted as a 32-bit `SBFM` word
instead of being rejected.

The five passing properties independently confirm the encoding is otherwise
correct for both widths and the full register range, cross-checked against the
reference assembler `llvm-mc-18`.

## Modules Tested

| Module | Target | Oracle | Properties | Result |
|---|---|---|---|---|
| `data_processing::sxtb_props` | `encode_sxtb` (data_processing.rs:872) | differential (llvm-mc-18 reference constant) + field placement + width-invariance + negative contract (arity / operand type / mixed width) | 6 | 5 pass / 1 fail |

## Bugs Found

**`encode_sxtb` silently accepts an architecturally invalid mixed-width source operand** —
full report: `pbt-out/bug_reports/encode_sxtb_silent_mixed_width_source.md`.

`sf`/`N` are derived only from `Rd` (operand 0); the source `Rn` width is bound
to `_` and discarded, so `sxtb w0, x0` encodes as a 32-bit `SBFM w0, w0, #0, #7`
word (`0x13001C00`) instead of erroring. Per the ARMv8 ARM, `SXTB` is a `SBFM`
alias and the source/destination must be the same width (`N == sf`); llvm-mc-18
rejects `sxtb w0, x0` with "error: invalid operand for instruction".

The defect is **one-directional**: the reverse form `sxtb x0, w0` (64-bit
destination, 32-bit source) is genuinely valid — llvm-mc-18 canonicalizes
`sxtb x0, x0` to `sxtb x0, w0` (both → `0x93401c00`) — so the source width
legitimately does not matter for a 64-bit destination. Property
`sxtb_source_width_irrelevant_for_64bit_destination` verifies this and passes.
This verified-correct behavior bounds the bug and is intentionally **not** filed
as a defect.

Witness (failing, shrunk PBT property):
- **property:** `sxtb_rejects_w_destination_with_x_source` (mod `sxtb_props`,
  data_processing.rs:7243)
- **reproduce:** `cargo test --lib data_processing::sxtb_props::sxtb_rejects_w_destination_with_x_source`
- **Falsifiable / minimal failing input:** `n = 0` (successes before failure: 0)
- **counterexample:** `ops = [Reg("w0"), Reg("x0")]` → `sxtb w0, x0`
- **actual:** `Ok(EncodeResult::Word(318774272))` (= `0x13001C00`, `sf = 0`)
- **expected:** `Err`

The same defect class (`let (rn, _) = get_reg(...)`) affects the sibling
sign/zero-extend encoders `encode_sxth` (line 863, already reported in
`encode_sxth_silent_mixed_width_source.md`), `encode_uxth`, and `encode_uxtb`;
each needs the same one-directional width guard.

## Design Caveats

- **Register 31 encodes as the zero register (XZR/WZR) and is valid for SXTB.**
  The generated register range (`0u32..=31`) includes 31; P1/P2 verify it encodes
  correctly (e.g. `sxtb xzr, wzr` → `0x93401fff`, cross-checked against
  llvm-mc-18). This is an intentional, codebase-wide design — register 31 is the
  zero register for non-load/store/non-add-sub instructions like `SBFM`/`SXTB`.
  *Doc evidence:* `src/backend/arm/assembler/encoder/mod.rs:135` —
  `parse_reg_num` maps `"xzr" | "wzr" => Some(31)`.

## Test Files Created

| File | Change | Type |
|---|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` | appended `#[cfg(test)] mod sxtb_props` (~135 lines, 6 `proptest!` properties) | inline PBT module |

No new top-level test files; the suite was appended as a sibling
`#[cfg(test)] mod sxtb_props` to match the file's existing convention
(`mod smull_props`, `mod sxth_props`, `mod madd_props`, …).

## Output Directories

| Path | Contents |
|---|---|
| `pbt-out/bug_reports/encode_sxtb_silent_mixed_width_source.md` | Bug report for the mixed-width finding (failing-property witness) |
| `pbt-out/REPORT.md` | This report |
