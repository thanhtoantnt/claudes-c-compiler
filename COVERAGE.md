# PBT Coverage: `encode_movz`

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_movz` (line 201)
**Target:** ARMv8 MOVZ (wide immediate) encoder — `sf 10 100101 hw imm16 Rd`

## Properties (6)

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| 1 | `movz_field_placement` — every fixed/variable field in spec position for in-range imm | reference | ✅ PASS |
| 2 | `movz_imm16_is_low_16_bits` — imm16 field == `imm & 0xFFFF` for any magnitude | reference | ✅ PASS |
| 3 | `movz_hw_tracks_lsl_shift_amount` — `lsl #N` ⇒ hw == N/16 (valid multiples of 16) | reference | ✅ PASS |
| 4 | `movz_rejects_out_of_range_immediate` — imm ≥ 0x10000 ⇒ `Err` | negative/error contract | ❌ FAIL |
| 5 | `movz_rejects_non_multiple_of_16_shift` — non-multiple-of-16 lsl ⇒ `Err` | negative/error contract | ❌ FAIL |
| 6 | `movz_w_reg_rejects_32_or_48_shift` — `lsl #32/#48` on `W` reg ⇒ `Err` | negative/error contract | ❌ FAIL |

## Findings

Three confirmed bugs (see `BUG_REPORT.md`). The positive properties document that the
encoder places fields correctly **when given valid input**, while the negative-contract
properties expose that it performs no validation on the immediate magnitude, the shift
amount, or the register-width/shift combination — silently masking/truncating all three.
The defects are systemic: `encode_movk` (line 234) and `encode_movn` (line 266) share the
identical masking (`& 0xFFFF`) and shift-normalization (`amount / 16`) code.
