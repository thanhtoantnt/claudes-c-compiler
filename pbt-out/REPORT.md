# REPORT — `encode_extr`

## Summary

Generated a property-based test suite for `encode_extr`
(`src/backend/arm/assembler/encoder/bitfield.rs:133`), the AArch64 `EXTR` (Extract
register) encoder. Six properties were written; **five pass, one fails**. The
single failing property is a confirmed SUT violation surfaced by a shrunk PBT input
(`minimal failing input: is_64 = true, big_lsb = 64, mid_lsb = 32, neg_imm = -1`).
The passing canonical-encoding property (`prop_extr_canonical_encoding`) validates
the oracle against the well-known reference `EXTR x0, x1, x2, #5 = 0x93C21420`, so
the negative-contract failure is a genuine encoder defect, not a spec error in the
test. (No AArch64 assembler — `llvm-mc` / `aarch64-linux-gnu-as` — is installed in
this environment, so the oracle is the ARM ARM structural layout plus the canonical
constant, cross-checked by field extraction of the emitted word.)

`encode_extr` is broken on its immediate range: it reads `lsb` with
`get_imm(...)? as u32` and ORs `lsb << 10` into the 6-bit `imms` field
(`[15:10]`) with **no range validation**. The ARM ARM constrains `lsb` to
`0..=63` for the 64-bit form and `0..=31` for the 32-bit form (where `imms[5]`
must be 0). Out-of-range values silently corrupt the encoding instead of being
rejected: `lsb = 64` sets bit 16 — the low bit of the `Rm` field — turning
`extr x0, x1, x2, #64` into `Ok(Word(0x13830020))` whose `Rm` reads back as 3
(not 2) and `imms` reads back as 0 (not 64). The five passing properties confirm
the *valid-input* encoding is otherwise exactly correct: sf/opc=00/`100111`/N==sf
/ o0=0/Rm/imms=lsb/Rn/Rd all land in their mandated positions, x{N} vs w{N} differ
only in sf[31]+N[22], the encoder is pure, and arity/operand-type errors are
rejected.

## Modules Tested

| Module | Target | Oracle | Properties | Result |
|---|---|---|---|---|
| `bitfield::prop_encode_extr_tests` | `encode_extr` (bitfield.rs:133) | reference constant (`0x93C21420`) + ARM ARM structural field placement + width differential + purity + negative contract (arity/operand type, and immediate range) | 6 | 5 pass / 1 fail |

## Bugs Found

**Bug 1 — `encode_extr` silently accepts out-of-range `#lsb`, corrupting the `imms`/`Rm`/`N` fields.**
Full report: `pbt-out/bug_reports/encode_extr_no_lsb_range_validation.md`.

`encode_extr` casts the `lsb` immediate to `u32` and shifts it into the 6-bit
`imms` field (`[15:10]`) with no bounds check. For `lsb = 64`, `(64u32 << 10)` is
bit 16, the low bit of `Rm`, so `extr x0, x1, x2, #64` returns
`Ok(Word(327352352))` = `0x13830020` (Rm reads back as 3, not 2; imms reads back
as 0, not 64). The 32-bit (W) form additionally accepts `32 <= lsb <= 63`, which
sets `imms[5]` — an architecturally UNDEFINED encoding. Negative `lsb` wraps via
the `i64 as u32` cast. GNU `as` rejects these with `immediate out of range`.

- **Falsifiable / minimal failing input:** `is_64 = true, big_lsb = 64, mid_lsb = 32, neg_imm = -1` (counterexample: `extr x0, x1, x2, #64`)
- **property:** `prop_rejects_out_of_range_lsb` (mod `prop_encode_extr_tests`, bitfield.rs:2620)
- **reproduce:** `cargo test --lib prop_encode_extr_tests::prop_rejects_out_of_range_lsb`
- **actual:** `Ok(EncodeResult::Word(327352352))` (`0x13830020`, Rm=3 imms=0)
- **expected:** `Err`

## Design Caveats

- **Register 31 (encoded as XZR/WZR) is a valid `EXTR` operand; the generated
  register range `0u32..=30` deliberately excludes it only to match the codebase
  convention of every sibling `prop_encode_*_tests` module in this file (none of
  `prop_encode_ubfx_tests` / `prop_encode_ubfm_tests` / `prop_encode_sbfm_tests` /
  `prop_encode_bfm_tests` exercise register 31). Register 31 flows through
  `encode_extr` as register-number 31 with nothing special beyond zero-register
  aliasing, and `EXTR Xd, Xn, Xm, #lsb` is architecturally valid with XZR/WZR in
  any operand position, so excluding it is a test-scope choice that does not mask
  a defect rather than a behavioral assertion.
  *Doc evidence:* `src/backend/arm/assembler/encoder/mod.rs:135` —
  `parse_reg_num` maps `"xzr" | "wzr" => Some(31)`; asserted by the existing
  unit test at `src/backend/arm/assembler/encoder/mod.rs:1066-1067`
  (`assert_eq!(parse_reg_num("xzr"), Some(31))` /
  `assert_eq!(parse_reg_num("wzr"), Some(31))`).

No other behavioral caveat is asserted. The absence of an AArch64 reference
assembler in this environment is a *methodology* limitation (the oracle falls back
to the ARM ARM structural layout + the canonical constant `0x93C21420`, validated
by the passing `prop_extr_canonical_encoding`), not an intentional-behavior claim
about the SUT, so it is not listed as a Design Caveat.

## Test Files Created

| File | Change | Type |
|---|---|---|
| `src/backend/arm/assembler/encoder/bitfield.rs` | appended `#[cfg(test)] mod prop_encode_extr_tests` (~245 lines, 5 `proptest!` properties + 1 standalone `#[test]` reference check) | inline PBT module |

No new top-level test files; the suite was appended as a sibling
`#[cfg(test)] mod prop_encode_extr_tests` to match the file's existing convention
(`mod prop_encode_ubfx_tests`, `mod prop_encode_ubfm_tests`,
`mod prop_encode_sbfm_tests`, `mod prop_encode_bfm_tests`, …).

## Output Directories

| Path | Contents |
|---|---|
| `pbt-out/bug_reports/encode_extr_no_lsb_range_validation.md` | Bug 1: out-of-range `#lsb` silently accepted; `lsb<<10` overflows `imms` into `Rm`/`N` (failing-property witness) |
| `pbt-out/REPORT.md` | This report |
