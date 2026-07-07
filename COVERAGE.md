# PBT Coverage

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_add_sub`
**Oracle type:** spec-field-extraction (ARMv8 ADD/SUB bit-layout invariant). The encoder is a pure bit-packer, so the ARMv8 manual defines the expected bit fields; each property decodes the returned word and asserts the fields.

## Properties (6) — all PASS at 2000 cases each

| # | Property | What it pins |
|---|----------|--------------|
| 1 | `add_imm_unshifted_field_placement` | `ADD Xd,Xn,#imm` (0..0xFFF): sf/op/S/opcode `10001`/sh=0/imm12/rn/rd all correct |
| 2 | `register_form_field_placement` | `ADD/SUB Xd,Xn,Xm`: opcode `01011`, op bit, rm/rn/rd, zero shift |
| 3 | `negative_immediate_flips_op` | `ADD #-N → SUB #N`, `SUB #-N → ADD #N`: op bit flips, imm12 = magnitude |
| 4 | `shifted_immediate_uses_sh_bit` | both explicit `lsl #12` and auto-shift → sh=1, imm12 = chunk |
| 5 | `unencodable_immediate_returns_err` | nonzero low-12 imm > 0xFFF → `Err` (no silent truncation) |
| 6 | `sf_bit_tracks_register_width` | bit 31 = 0 for W regs, 1 for X regs |

## Untested branches (out of scope / noted)

- NEON vector form (`RegArrangement` first operand → `encode_neon_add_sub`).
- Relocation-modifier forms (`:lo12:`, `:tprel_lo12_nc`, `:tprel_hi12`).
- Extended-register form (`sxtw`/`uxtw`/…) and the SP-aware extended fallback.
- `set_flags` (ADDS/SUBS) is only touched tangentially (S bit asserted =0 in the no-flags paths); a dedicated ADDS/SUBS property was not added since it shares the same field layout with S=1.

---

# PBT Coverage — `sysreg_encoding`

**Target:** `src/backend/arm/assembler/encoder/system.rs` → `sysreg_encoding`
**Oracle type:** field-extraction round-trip (inverse of bit-packing). The function packs five AArch64 system-register addressing fields into a 16-bit encoding used by MRS/MSR: `op0[15:14] | op1[13:11] | CRn[10:7] | CRm[6:3] | op2[2:0]`, masking each to its declared width. Each property extracts a field back out and asserts it equals the masked input, independently pinning both masking and placement.

## Properties (5) + 1 deterministic companion — all PASS

| # | Property | What it pins |
|---|----------|--------------|
| 1 | `sysreg_output_fits_in_16_bits` | result always ≤ 0xFFFF regardless of input magnitude |
| 2 | `sysreg_ignores_high_bits_of_inputs` | encoding(raw) == encoding(masked): high bits of each input are dropped |
| 3 | `sysreg_each_field_roundtrips_at_expected_position` | each of op0/op1/CRn/CRm/op2 decodes back at its (shift, mask) |
| 4 | `sysreg_injective_over_valid_ranges` | distinct legal tuples → distinct encodings (proves fields disjoint) |
| 5 | `sysreg_changing_one_field_isolates_to_its_bits` | mutating one field changes only its bits; others untouched |
| C | `sysreg_known_canonical_values` | SCTLR_EL1=S3_0_C1_C0_0→0xC080, all-zeros→0, all-max→0xFFFF |

## Notes

- Function is a pure, branchless bit-packer with no error path; the strongest meaningful oracle is the inverse (field extraction). `proptest::any::<u32>()` is used for every field so masking invariance is checked over the full u32 range, including values far outside the legal widths.
- Already indirectly covered by the existing `proptest_msr::msr_generic_sysreg_matches_sysreg_encoding` differential test; this module covers the function directly.
- No bugs found in `sysreg_encoding` itself. The one initial failure was a wrong canonical-tuple constant (SCTLR_EL1 op0 is 3, not 2) in the *test fixture*, now corrected.
