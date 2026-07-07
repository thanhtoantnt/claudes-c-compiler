# PBT Coverage — `encode_mul`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Result:** 6/6 properties pass. **1 functional bug found** (silent SP→XZR aliasing) — see Bugs Found.

## What the function does
`MUL Rd, Rn, Rm` is encoded as `MADD Rd, Rn, Rm, XZR`:
```
word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd
```
i.e. ARMv8 MADD `sf 0 0 11011 000 Rm 0 Ra Rn Rd` with `Ra = XZR = 0b11111` and `o0 = 0`.
A leading `RegArrangement` first operand delegates to `encode_neon_mul`.

## Properties verified
1. `mul_xregs_field_placement` — spec-exact placement of every fixed + register field (reference).
2. `mul_sf_tracks_width` — `sf` (bit 31) reflects X vs. W (reference).
3. `mul_equals_madd_with_xzr` — bit-identical to `encode_madd(.., XZR)` for all widths/operands (differential oracle — the strongest check).
4. `mul_width_only_flips_sf_bit` — width affects only bit 31 (algebraic invariant).
5. `mul_rejects_too_few_operands` — <3 operands → `Err` (negative contract).
6. `mul_rejects_immediate_operand` — non-register Rm → `Err` (negative contract).

## Bugs Found

### BUG-1 (High): `encode_mul` silently accepts SP in any operand → encoded as multiply-by-zero
`mul x0, x1, sp` is accepted with `Ok(Word(...))` whose Rm field is 31 — i.e. it is silently
encoded as `mul x0, x1, xzr` (a multiply-by-zero). ARMv8 MADD/MUL has **no** SP-using variant;
field 31 is XZR, and SP in these operands is UNPREDICTABLE/unallocated. Root cause: the shared
`get_reg`→`parse_reg_num` helper maps `sp`/`wsp`→31 unconditionally (correct for SP-aware
ADD/SUB, wrong for every XZR-only data-processing instruction). Confirmed empirically via the
characterization test `mul_sp_in_rm_is_silently_accepted_as_xzr`; same aliasing hits SP in Rd
and Rn too, and sibling encoders (`encode_madd`/`encode_div`/`encode_logical` reg form/etc.).

- **Report:** `pbt-out/bug_reports/encode_mul_sp_operand_silently_accepted_as_xzr.md`
- **Repro:** `cargo test --lib backend::arm::assembler::encoder::data_processing::tests::mul_sp_in_rm_is_silently_accepted_as_xzr -- --nocapture`
- **Evidence:** `mul x0,x1,sp -> Ok(Word(2602531872))`, Rm field = 31 (XZR)
- **Suggested fix:** reject SP/WSP (and mixed widths) in `encode_mul` before encoding, or
  add a `get_reg_no_sp` helper used by all XZR-only data-processing encoders.

---

# PBT Coverage — `encode_madd`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_madd`
**Result:** 5/5 properties pass. **1 functional finding** (silent mixed-width operand acceptance) — see BUG-2.

## What the function does
Encodes ARMv8-A `MADD <Rd>,<Rn>,<Rm>,<Ra>` as
`sf 0 0 11011 000 Rm o0 Ra Rn Rd` with `o0 = 0`:
```
word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (ra << 10) | (rn << 5) | rd
```
The `o0` bit (bit 15) is 0 for MADD and 1 for MSUB; `sf` is taken from operand 0 (Rd).

## Properties verified
1. `madd_field_placement` — spec-exact placement of every fixed field (bits 30:21 = `0b0011011000`,
   `o0 = 0`) and every register field (Rd/Rn/Rm/Ra) for both widths, Ra=31 (xzr) allowed.
2. `madd_msub_differ_only_in_o0` — `encode_madd ⊕ encode_msub == 1<<15` for all inputs
   (differential oracle against the sibling MSUB encoder).
3. `madd_width_only_flips_sf_bit` — X↔W swap of identical numbers changes only bit 31 (invariant).
4. `madd_rejects_too_few_operands` — `<4` operands → `Err` (negative contract).
5. `madd_ra_xzr_equals_mul` — `MADD Rd,Rn,Rm,XZR` is bit-identical to `encode_mul` (algebraic /
   reference oracle for the `MUL` alias).

## Bugs Found

### BUG-2 (Medium): `encode_madd` silently accepts mixed-width operands (sf taken from Rd only)
The encoder derives `sf` (bit 31) exclusively from operand 0 and never validates that all four
operands share the same width. `madd x0, w1, x2, x3` is accepted with `Ok(Word(0x9b028060)`)
whose `sf = 1` (64-bit) even though `Rn = w1` is a 32-bit register — an UNPREDICTABLE/
unallocated combination in AArch64. Same shape affects any data-processing encoder that calls
`get_reg(.., 0)` for width and ignores the `_` width of later operands (`encode_madd`,
`encode_msub`, `encode_mul`, `encode_div`, …). Confirmed by the characterization test
`madd_silently_accepts_mixed_width_operands`.

- **Repro:** `cargo test --lib madd_silently_accepts_mixed_width_operands -- --nocapture`
- **Evidence:** `madd x0,w1,x2,x3 -> Ok(Word(2600602656))`, `sf = 1, rn field = 1 (from w1)`
- **Suggested fix:** after resolving all four registers, assert their `is_64` flags agree and
  return `Err` on mismatch (a `get_reg_consistent` helper shared across this encoder family).

> Note: the SP→XZR silent aliasing documented in BUG-1 also applies to `encode_madd`'s operands
> (same `get_reg` root cause); it is not re-listed here.
