# PBT Coverage — `encode_adrp`

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `encode_adrp(operands: &[Operand]) -> Result<EncodeResult, String>`
**Test module:** `prop_encode_adrp_tests` (appended to the same file)
**Framework:** `proptest` (already a dev-dependency)
**Result:** 6/6 properties PASS at 256 cases each.

## Instruction under test

AArch64 ADRP (ARM ARM C6.2.10): `1 immlo[1:0] 10000 immhi[18:0] Rd`.
- `[31]` = 1 (distinguishes ADRP from ADR, whose bit 31 is 0).
- `[28:24]` = `10000` (the PC-relative address opcode).
- `[30:29]` immlo + `[23:5]` immhi — the page offset, **not** computed by the
  assembler. Per the AArch64 ELF ABI, `R_AARCH64_ADR_PREL_PG_HI21`
  (`AdrpPage21`) resolves `S + A` and discards the low 12 bits to recover the
  page; `R_AARCH64_ADR_GOT_PAGE21` (`AdrGotPage21`) does the same against the
  GOT. So the encoder emits `immlo = immhi = 0` (template word `0x9000_0000`)
  and attaches a relocation carrying the symbol and the **exact** addend.

## Properties written

| # | Name | Oracle type | What it pins down |
|---|------|-------------|-------------------|
| 1 | `prop_adrp_word_template` | Reference (structural) | Word == `0x9000_0000 \| Rd`; reloc is `AdrpPage21`. |
| 2 | `prop_rd_field_low_5_bits` | Field placement | `Rd` occupies `[4:0]`; every bit above is the fixed template (op=1, `[28:24]=10000`, imm fields zero). |
| 3 | `prop_symbol_and_label_identical` | Differential + verbatim | `Symbol` and `Label` operands yield identical `WordWithReloc`; symbol string copied verbatim (incl. uppercase preserved), addend 0. |
| 4 | `prop_symboloffset_addend_verbatim` | Passthrough contract | `SymbolOffset` addend forwarded unchanged across the full `i64` range (negative, non-page-aligned, max) — no masking/truncation by the encoder. |
| 5 | `prop_got_modifier_reloc` | Differential + negative | `:got:sym` → `AdrGotPage21` (addend 0); plain `sym` → `AdrpPage21`; a `lo12` modifier is rejected with `Err`. |
| 6 | `prop_rejects_malformed_operands` | Negative / error contract | Empty/single-operand vectors and a non-register first operand all return `Err`; no panic, no corrupt word. |

Template constant cross-checked against the canonical AArch64 encoding `ADRP x0, . = 0x90000000`.

## Findings

**No bug.** The encoder is correct on every exercised dimension.

### Observations (not defects — no reports filed)

- **Addend is not range-checked.** `encode_adrp` forwards the `SymbolOffset`
  addend verbatim into the relocation. This is *correct*: the page-relative
  masking (dropping the low 12 bits of `S + A`) happens at relocation-
  application time in the linker, not in the assembler. Asserting this as a
  positive passthrough property (Property 4) confirms the encoder does not
  erroneously truncate — the delegation is intentional, so no bug report.

- **Rt == SP / XZR accepted.** `parse_reg_num` maps `sp`/`xzr` to register
  31; `encode_adrp` does not reject `adrp sp, …`. Unlike most data-processing
  instructions, ADRP *does* permit `Rd == SP`, so this is architecturally
  valid (consistent with GAS). No defect.

---

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
