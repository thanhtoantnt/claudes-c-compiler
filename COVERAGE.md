# PBT Coverage — `encode_add_sub`

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
