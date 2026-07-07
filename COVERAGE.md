# PBT Coverage: `encode_branch`

**File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
**Function:** `encode_branch` (line 418)
**Target:** AArch64 unconditional `B` (branch) encoder — `000101 imm26`, emits a `Jump26` relocation.

## Properties (5)

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| 1 | `prop_opcode_structure_and_imm26_zero` — word is exactly `0x14000000`: opcode `0b000101` in bits [31:26], linker-reserved imm26 [25:0] zero | reference | ✅ PASS |
| 2 | `prop_branch_vs_bl_differ_only_bit31` — `encode_branch` XOR `encode_bl` == `1 << 31` | differential | ✅ PASS |
| 3 | `prop_reloc_is_jump26_primary_forms` — `Symbol`/`SymbolOffset` ⇒ `Jump26` reloc with exact symbol & addend | reference | ✅ PASS |
| 4 | `prop_symbol_forwarding_all_accepted_kinds` — symbol/addend forwarded verbatim across every `get_symbol`-accepted operand kind (Symbol/Label/SymbolOffset/Modifier/ModifierOffset/Reg/Cond/Barrier) | reference | ✅ PASS |
| 5 | `prop_rejects_non_symbol_operands` — Imm/Mem/MemExpr/MemRegOffset/Shift/Extend/Expr/RegArrangement/RegLane ⇒ `Err` | negative/error contract | ✅ PASS |

## Findings

None. `encode_branch` is a trivial relocation-emitting encoder: the opcode is a compile-time constant (`0x14000000`), the offset is left entirely to the linker (no immediate to mask/truncate), and the relocation metadata (type, symbol, addend) is forwarded unchanged from `get_symbol`. The negative-contract property confirms the encoder correctly refuses non-symbol targets rather than silently encoding them. No immediate-magnitude or shift-validation concerns apply to this instruction class.

---

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
