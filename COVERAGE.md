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

---

# PBT Coverage — `encode_sys`

**Target:** `src/backend/arm/assembler/encoder/system.rs` → `encode_sys`
**Oracle type:** field-extraction round-trip (inverse of bit-packing). The function builds the AArch64 `SYS #op1, Cn, Cm, #op2 [, Xt]` instruction as `0xD508_0000 | (op1&7)<<16 | (CRn&0xF)<<12 | (CRm&0xF)<<8 | (op2&7)<<5 | Rt`, parsing a comma-separated operand string. Each property extracts a field back out of the result and asserts it equals the masked input.

## Properties (6) + 1 deterministic companion — all PASS

| # | Property | What it pins |
|---|----------|--------------|
| 1 | `sys_high_opcode_bits_fixed` | bits[31:19] == 0xD508_0000 for every well-formed input |
| 2 | `sys_each_field_roundtrips_in_valid_range` | op1/CRn/CRm/op2/Rt each decode back at their declared (shift, mask) |
| 3 | `sys_masks_field_inputs_to_width` | encoding(raw) == encoding(masked): high bits of each numeric input dropped |
| 4 | `sys_omitted_register_defaults_to_xzr_31` | 4-operand form sets Rt = 31 (xzr) |
| 5 | `sys_distinct_valid_tuples_distinct_words` | distinct legal tuples → distinct words (fields disjoint) |
| 6 | `sys_rejects_malformed_operands` | <4 operands, non-numeric op1/op2, unparseable register → Err |
| C | `sys_canonical_words` | all-zero fields→0xD508_001F, DC-CIVAC fields, uppercase CRn/CRm parity |

## Notes

- Function is a pure bit-packer over a parsed string; the inverse (field extraction) is the strongest meaningful oracle and fully reconstructs the encoding jointly with property 1.
- Masking invariance (property 3) is checked over inputs up to 0xFFFF — well outside each 3-/4-bit field width — confirming the `& 7` / `& 0xF` masks.
- Error contract (property 6) covers all four failure paths: arity, op1 parse, op2 parse, and register parse.
- No bugs found in `encode_sys`.

## `encode_neon_ext` (src/backend/arm/assembler/encoder/neon.rs)

Property suite in `mod ext_pbt_tests` — 6 properties, all PASS (proptest default 256 cases each).

- **prop_matches_reference** — differential oracle: encoder word == independent reconstruction of the `0 Q 101110 00 0 Rm 0 imm4 0 Rn Rd` layout, for all rd/rn/rm in 0..32 and index 0..16, both `.8b` and `.16b`.
- **prop_fixed_opcode_fields** — bit31==0, bits29-24==0b101110, and the always-zero filler bits 10/15/21/22/23 are clear.
- **prop_q_bit_selects_16b** — Q (bit 30) is 1 iff arrangement == "16b".
- **prop_register_fields_preserved** — Rd (4-0), Rn (9-5), Rm (20-16) round-trip the inputs.
- **prop_imm4_field_masks** — bits 14-11 == index & 0xF; for index < 16, equal to index. Documents that the encoder performs no range validation (index is masked, not rejected).
- **prop_error_contracts** — <4 operands or a non-Imm 4th operand return Err.

No bugs found. Notable observation (not a defect, recorded): the function does not validate the byte-index range (ARM allows 0-15) nor the arrangement string; any index is silently truncated to 4 bits and any non-`16b` string maps to Q=0. The `prop_imm4_field_masks` property pins this current behavior.

## `encode_neon_shift_imm` (src/backend/arm/assembler/encoder/neon.rs)

Property suite in `mod shift_imm_pbt_tests` — 7 properties, all PASS (2048 cases each).

- **prop_fixed_fields** — for every valid shift, bit31==0, bits[28:23]==0b011110, bits[15:10]==0b000001, and U (bit29)==1.
- **prop_reg_fields_preserved** — Rd (4:0) and Rn (9:5) round-trip the source register numbers.
- **prop_q_bit_per_arrangement** — Q (bit30) is 1 for wide forms (16b/8h/4s/2d), else 0.
- **prop_immh_immb_oracle** — differential oracle: encoded immh:immb (bits[22:16]) == `2*elem_bits - shift`, and the shift is fully reconstructable from the word.
- **prop_is_unsigned_ignored** — documents a design gap: the `_is_unsigned` parameter is ignored; `is_unsigned=true` and `false` produce identical words and U is hardcoded to 1, so the function cannot emit an SSHR (U=0) encoding.
- **prop_error_contracts** — <3 operands, unsupported arrangements (1d/1q/2h/3s/empty), and a non-Imm 3rd operand all return Err.
- **prop_shift_zero_accepted_but_unallocated** — characterization: shift==0 is silently accepted (no range check) and yields immh==0, which is UNALLOCATED in ARMv8.

### Findings / latent gaps (not crashes; current behavior pinned by tests)

1. **`_is_unsigned` is dead** — the only caller-facing knob is ignored; USHR vs SSHR cannot be selected. Likely the caller should branch U on this flag (compare `encode_neon_ushr` which hardcodes U=1 vs `encode_neon_sshr` which hardcodes U=0).
2. **No shift-range validation** — shift=0 (and shifts in `(elem_bits, 2*elem_bits]`) are accepted, producing reserved/UNALLOCATED encodings (immh=0, or a wrong element-size immh). Shifts beyond `2*elem_bits` would **panic** in debug builds via unsigned-integer underflow in `16 - shift as u32` (before masking), so the function is not panic-safe for arbitrary `i64` immediates.
