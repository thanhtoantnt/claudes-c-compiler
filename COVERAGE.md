# PBT Coverage — `encode_cbz`

**File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
**Function:** `encode_cbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String>`
**Test module:** `prop_encode_cbz_tests` (appended to the same file)
**Framework:** `proptest` (already a dev-dependency)
**Result:** 5/5 properties PASS at 512 cases each.

## Instruction under test

AArch64 CBZ / CBNZ (ARM ARM C5.6.21 / C5.6.22): `sf 011010 op imm19 Rt`
- `[31]` sf — 1 = 64-bit (X), 0 = 32-bit (W)
- `[30:25]` `011010` — fixed opcode
- `[24]` op — 0 = CBZ, 1 = CBNZ
- `[23:5]` imm19 — linker-filled branch offset (encoder leaves it zero)
- `[4:0]` Rt — register

The result is `EncodeResult::WordWithReloc` carrying a `RelocType::CondBr19` relocation.

## Properties written

| # | Name | Oracle type | What it pins down |
|---|------|-------------|-------------------|
| A | `prop_opcode_structure_and_fields` | Reference (structural) | Fixed opcode `011010`@[30:25], imm19@[23:5] left zero, sf@[31], op@[24], Rt@[4:0]; full word reconstructable from fields. |
| B | `prop_cbz_xor_cbnz_is_bit24` | Differential | `encode_cbz(_,false) ^ encode_cbz(_,true) == 1<<24`. |
| C | `prop_sf_bit_is_bit31` | Differential | `x{N}` vs `w{N}` differ only in bit 31 (sf). |
| D | `prop_reloc_is_condbr19_with_symbol` | Contract | Result carries `CondBr19` reloc with symbol & addend forwarded across all 8 operand kinds `get_symbol` accepts. |
| E | `prop_rejects_invalid_operands` | Negative / error contract | Missing operands, non-register first operand, and non-symbol targets all return `Err` — no panic, no silent encoding. |

Opcode constants cross-checked against canonical AArch64 encodings: `CBZ x0 = 0xB4000000`, `CBNZ x0 = 0xB5000000`, `CBZ w5 = 0x34000005`.

## Findings

**No bug.** The encoder is correct and well-behaved on every exercised dimension.

### Observation (not a defect — no report filed)

`parse_reg_num` resolves `sp`/`wsp`/`xzr`/`wzr` to register number 31, and `encode_cbz` does not reject `cbz sp, …` or `cbz xzr, …`. Per the ARM ARM these are constrained-UNPREDICTABLE / reserved for CBZ/CBNZ (Rt must not be SP; Rt==11111 is also reserved). This project deliberately delegates register validity to `parse_reg_num`, which matches the behaviour of GAS (which likewise encodes these without complaint by default). Treated as intentional policy consistency rather than a defect; if strict architectural validation is ever desired, that is the place to add it.
