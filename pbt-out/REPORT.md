# REPORT — `encode_uxtw`

## Summary

Generated a property-based test suite for `encode_uxtw`
(`src/backend/arm/assembler/encoder/data_processing.rs:881`), the AArch64 `UXTW`
encoder. Five properties were written; **three pass, two fail**. Both failing
properties are confirmed SUT violations surfaced by failing, shrunk PBT inputs
(`minimal failing input: rd = 0, rn = 0` for each), cross-checked against the
reference assembler `llvm-mc-18 --triple=aarch64 --show-encoding` (the differential
oracle for every expected constant below).

`encode_uxtw` is broken: it emits the encoding of `MOV Wd, Wn` (`ORR Wd, WZR, Wn`
= `0x2A0003E0 | (rn << 16) | rd`) instead of the canonical `UBFM` form for `UXTW`,
and silently accepts the architecturally invalid 32-bit form `uxtw Wd, Wn`. The
ARMv8 ARM defines no standalone `UXTB`/`UXTH`-style alias for a 32-bit `uxtw`; the
only valid standalone spelling is the 64-bit `uxtw Xd, Wn`, which llvm-mc-18
encodes as `UBFM Xd, Xn, #0, #31` (`0xD3407C00 | (rn << 5) | rd`). This is
structurally different from the sibling encoders `encode_uxtb` / `encode_uxth`,
which correctly emit the canonical `UBFM` alias.

The three passing properties confirm the *implementation's* actual contract (the
word it emits is a well-formed `ORR Wd, WZR, Wn` / `MOV` alias with correct field
placement, widths ignored) and that arity / operand-type errors are rejected.

## Modules Tested

| Module | Target | Oracle | Properties | Result |
|---|---|---|---|---|
| `data_processing::uxtw_props` | `encode_uxtw` (data_processing.rs:881) | differential (llvm-mc-18 reference constant) + implementation-contract field placement + negative contract (arity / operand type) | 5 | 3 pass / 2 fail |

## Bugs Found

**Bug 1 — `encode_uxtw` emits `MOV Wd, Wn` instead of canonical `UBFM` for the valid form.**
Full report: `pbt-out/bug_reports/encode_uxtw_emits_mov_not_ubfm.md`.

For the only spec-valid form `uxtw Xd, Wn`, llvm-mc-18 encodes
`UBFM Xd, Xn, #0, #31` (`0xD3407C00 | (rn << 5) | rd`). `encode_uxtw` instead
emits `0x2A0003E0 | (rn << 16) | rd` — a 32-bit `mov Wd, Wn`. The function's own
header comment records the correct `UBFM` form but the implementation chose the
wrong branch (supporting evidence: `data_processing.rs:883` documents
`// Or: UBFM Xd, Xn, #0, #31`, while `data_processing.rs:886` documents
`// Use 32-bit ORR (MOV alias)` — the branch actually taken).

- **Falsifiable / minimal failing input:** `rd = 0, rn = 0` (counterexample: `uxtw x0, w0`)
- **property:** `uxtw_canonical_encoding_for_xd_wn` (mod `uxtw_props`, data_processing.rs:7642)
- **reproduce:** `cargo test --lib data_processing::uxtw_props::uxtw_canonical_encoding_for_xd_wn`
- **actual:** `Ok(EncodeResult::Word(0x2A0003E0))` (32-bit `mov w0, w0`)
- **expected:** `Ok(EncodeResult::Word(0xD3407C00))` (`UBFM x0, x0, #0, #31`, per llvm-mc-18)

**Bug 2 — `encode_uxtw` silently accepts the invalid 32-bit form `uxtw Wd, Wn`.**
Full report: `pbt-out/bug_reports/encode_uxtw_silent_32bit_destination.md`.

`uxtw Wd, Wn` is not a valid AArch64 instruction (`UXTW` has no 32-bit destination
alias); llvm-mc-18 rejects it with "error: invalid operand for instruction".
`encode_uxtw` accepts it because both `get_reg` width flags are bound to `_` and
discarded (the same width-ignoring mechanism that makes `uxtw x0, w1`, `uxtw w0,
w1`, and `uxtw x0, x1` all produce the identical word), emitting `mov w0, w1`
instead of erroring.

- **Falsifiable / minimal failing input:** `rd = 0, rn = 0` (counterexample: `uxtw w0, w0`)
- **property:** `uxtw_rejects_32bit_destination_form` (mod `uxtw_props`, data_processing.rs:7618)
- **reproduce:** `cargo test --lib data_processing::uxtw_props::uxtw_rejects_32bit_destination_form`
- **actual:** `Ok(EncodeResult::Word(0x2A0003E0))`
- **expected:** `Err`

## Design Caveats

- **Register 31 (encoded as WZR/XZR) is intentionally valid as a `uxtw` operand,
  so the generated register range `0u32..=31` is correct and not a test artifact.**
  For the canonical form, llvm-mc-18 confirms `uxtw x0, wzr` and `uxtw xzr, w0`
  are valid (`ubfx x0, xzr, #0, #32`, `ubfx xzr, x0, #0, #32`). Register 31 maps
  to the zero register codebase-wide for this instruction class.
  *Doc evidence:* `src/backend/arm/assembler/encoder/mod.rs:135` — `parse_reg_num`
  maps `"xzr" | "wzr" => Some(31)`; asserted by the existing unit test at
  `src/backend/arm/assembler/encoder/mod.rs:1065-1067`
  (`assert_eq!(parse_reg_num("xzr"), Some(31))`).

  (No other behavioral caveat is asserted here. The "operand width ignored"
  observation is not an intentional design — it is the root-cause mechanism of
  Bug 2 above, with no docstring stating it is intended. The "correct `UBFM` form
  recorded in a comment" observation is bug-supporting evidence for Bug 1, not an
  intentional-behavior caveat.)

## Test Files Created

| File | Change | Type |
|---|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` | appended `#[cfg(test)] mod uxtw_props` (~145 lines, 5 `proptest!` properties) | inline PBT module |

No new top-level test files; the suite was appended as a sibling
`#[cfg(test)] mod uxtw_props` to match the file's existing convention
(`mod smull_props`, `mod sxth_props`, `mod uxth_props`, `mod uxtb_props`, …).

## Output Directories

| Path | Contents |
|---|---|
| `pbt-out/bug_reports/encode_uxtw_emits_mov_not_ubfm.md` | Bug 1: wrong instruction (`MOV` not `UBFM`) for the valid `uxtw Xd, Wn` form (failing-property witness) |
| `pbt-out/bug_reports/encode_uxtw_silent_32bit_destination.md` | Bug 2: invalid `uxtw Wd, Wn` form silently accepted (failing-property witness) |
| `pbt-out/REPORT.md` | This report |
