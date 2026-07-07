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
