# PBT Coverage — `encode_adc`

Target: `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_adc`.

**Result: all properties pass. No findings.**

`encode_adc` is a pure register-to-register AArch64 encoder with no immediate,
shift, or relocation handling, so the usual encoder failure modes (silent
immediate truncation, shift clamping, missing range validation) do not apply.

## Properties added (in-module, `data_processing::tests`)

| # | Property | Oracle |
|---|----------|--------|
| 1 | `adc_field_placement` | Reference (ARMv8 ARM C4.1.4): sf=1, op=0, fixed opcode bits 28:21 = `0xD0`, reserved bits 15:10 = 0, Rm/Rn/Rd placed. |
| 2 | `adc_s_bit_tracks_set_flags` | Algebraic: S (bit 29) == `set_flags` (ADC vs ADCS), both widths. |
| 3 | `adc_sf_tracks_register_width` | Algebraic: sf (bit 31) == 1 for X, 0 for W. |
| 4 | `adc_vs_sbc_op_bit` | Differential: op bit is 0 for ADC, 1 for SBC across flags/width. |
| 5 | `adc_rejects_bad_operand_arities` | Negative contract: <3 operands or a non-register (Imm) operand → `Err`. |

## Verification notes
- All 5 `adc_*` tests pass (`cargo test --lib data_processing::tests::adc`).
- The 17 failures in the broader `data_processing` module are pre-existing
  rejection/range tests for *other* encoders (movz/movn/movk/neg/mvn/logical/shift)
  and are unrelated to `encode_adc`.

## encode_sbc (data_processing.rs) — 2026-07-08
Added 5 proptest properties + 1 deterministic reference-oracle test for
`encode_sbc` (ARMv8 Subtract-with-Carry). All pass (256 cases each).

- `sbc_known_constant_encoding`: independent oracle `sbc x0,x1,x2 == 0xDA020020`,
  `sbcs == 0xDA020020|(1<<29)`.
- `sbc_64bit_field_placement`: every fixed (sf, op, S, opcode bits 28..21,
  reserved bits 15..10) and variable (Rm/Rn/Rd) field matches the ARMv8 spec.
- `sbcs_flips_only_s_bit`: SBC vs SBCS differ only in bit 29.
- `sbc_is_adc_with_op_bit_set`: SBC ^ ADC == (1<<30) — the two carry
  instructions are structurally identical apart from the subtract op bit.
- `sbc_sf_tracks_register_width`: sf (bit 31) tracks W vs X register bank.
- `sbc_rejects_bad_operand_arities`: negative contract — <3 operands or an
  immediate in the 3rd slot returns Err (never silently encoded).

No finding: `encode_sbc` correctly emits `sf 1 S 11010000 Rm 000000 Rn Rd`,
reserved bits are 0, and arity errors are rejected via `get_reg`.
