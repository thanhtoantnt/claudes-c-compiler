# PBT Coverage — `encode_tbz`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, `encode_tbz`
**Framework:** proptest (Rust), inline `#[cfg(test)] mod prop_encode_tbz_tests`
**Status:** ✅ All 5 properties PASS (256 default + 2048-case verification run, no failures, no shrinks)

## Function under test

```rust
pub(crate) fn encode_tbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String>
```

Encodes AArch64 **TBZ** (`is_nz=false`) / **TBNZ** (`is_nz=true`):
`TBZ/TBNZ Rt, #bit, label`

Instruction-word layout: `b5 [31] · 011011 [30:25] · op [24] · b40 [23:19] · imm14 [18:5] · Rt [4:0]`
Emits a `WordWithReloc` carrying a `TstBr14` relocation for the 14-bit branch offset.

## Properties

| # | Property | Oracle | What it pins down |
|---|----------|--------|-------------------|
| A | `prop_opcode_structure_and_fields` | structural | fixed opcode bits `[30:25]=011011`, linker-reserved `imm14 [18:5]==0`, plus exact masks/positions of `b5`, `op`, `b40`, `Rt` |
| B | `prop_tbz_xor_tbnz_is_bit24` | differential | TBZ ⊕ TBNZ == `1<<24` (only `op` differs) |
| C | `prop_width_independent` | differential | `x{N}` and `w{N}` encode identically — TBZ has no `sf` bit (bit 31 is reused for `b5`) |
| D | `prop_bit_round_trips` | idempotent | `(b5<<5) | b40 == bit` for valid bit ∈ [0,63] |
| E | `prop_reloc_is_tstbr14_with_symbol` | contract | result is `WordWithReloc` with `RelocType::TstBr14`, symbol + addend forwarded verbatim (covers both `Symbol` and `SymbolOffset` operand forms) |

## Generators

- `arb_reg`: `n ∈ [0,30]`, `x`/`w` prefix (31 = sp/zr excluded — invalid as Rt here).
- `bit ∈ [0,63]`: the full valid AArch64 bit-position range.
- `arb_sym_operand`: identifier-shaped symbol, with/without addend ∈ [-4096, 4096].
- `is_nz`: bool.

## Notes / observations (not bugs)

- Register width is deliberately ignored by the encoder (`let (rt, _) = ...`); Property C documents this as the *intended* contract since bit 31 is occupied by `b5`.
- `bit` is read as `i64` then cast `as u32`; negative or ≥64 immediates wrap rather than erroring. Not exercised — out of the AArch64-valid input contract, and the cast is deterministic. A future hardening property could assert the encoder rejects `bit ∉ [0,63]`.

---

## `encode_neon_logical` — NEON ORR/AND/EOR (vector three-same)

File: `src/backend/arm/assembler/encoder/neon.rs` · Module `neon_logical_tests` · framework: proptest · **7/7 PASS** (4096 cases each).

| # | Property | Oracle | What it pins down |
|---|----------|--------|-------------------|
| 1 | `prop_matches_reference_encoding` | differential | word == independent reconstruction from `(Q,U,size,Rm,Rn,Rd)` |
| 2 | `prop_register_fields_preserved` | structural | `Rd[4:0]`, `Rn[9:5]`, `Rm[20:16]` survive untouched across full v0–v31 range |
| 3 | `prop_q_bit_only_for_16b` | contract | `Q==1` iff arrangement `== "16b"`; every other arrangement → `Q=0` |
| 4 | `prop_u_and_size_per_opc` | algebraic | `opc → (U,size)` table: AND(0,00), ORR(0,10), EOR(1,00); opc=0b11 aliases opc=0b10 |
| 5 | `prop_fixed_opcode_fields_constant` | structural | bit31=0, class[28:24]=01110, bit21=1, opcode[15:11]=00011, bit10=1 — invariant |
| 6 | `prop_source_arrangements_ignored` | differential | only operand 0's arrangement feeds Q; operands 1/2 arrangements discarded |
| 7 | `prop_error_contracts` | negative | <3 operands → Err; `opc ≥ 4` → Err |

### Notes / observations (not bugs)

- **opc=0b11 aliasing**: the source marks opc=0b11 "ANDS - not valid for NEON, fall back" and emits the *identical* `(U=1,size=00)` word as EOR (opc=0b10). Property 4 asserts this aliasing explicitly rather than treating it as a failure — it is a documented fall-back, not a corruption.
- **No arrangement validation**: the encoder only special-cases `"16b"→Q=1`; passing non-byte arrangements (`.4s`, `.2d`, …) yields `Q=0` instead of an error. ARMv8 restricts logical-vector ops to `.8b`/`.16b`, so such inputs produce an under-specified encoding. Property 3 characterizes this gap; a hardening property could assert rejection of non-byte arrangements.
- **Implicit operand-count check**: there is no explicit `if operands.len() < 3` guard; missing registers surface as an error from `get_neon_reg` instead. Property 7 covers the contract either way.

### Pre-existing failure (unrelated)

`neon::tbl_pbt_tests::prop_empty_list_does_not_panic` FAILS on the unmodified tree (verified via `git stash`) — it is a bug in the `tbl` encoder module, not introduced by this change.

---

## `encode_ccmp_ccmn` — CCMP/CCMN (immediate + register)

File: `src/backend/arm/assembler/encoder/compare_branch.rs` · Module `prop_ccmp_ccmn_tests` · framework: proptest · **5/6 PASS, 1 FAIL (finding)** (4096 cases each).

| # | Property | Oracle | What it pins down |
|---|----------|--------|-------------------|
| A | `prop_opcode_structure_and_fields` | structural | fixed opcode bits (set `0x3A400000`, zero `0x05A00410`); sf[31], op[30], cond[15:12], Rn[9:5], nzcv[3:0], o3[11], imm5/Rm[20:16] |
| B | `prop_ccmp_xor_ccmn_is_bit30` | differential | CCMP ⊕ CCMN == `1<<30` only |
| C | `prop_sf_bit_is_bit31` | differential | x{N} ⊕ w{N} == `1<<31` only |
| D | `prop_nzcv_masked_to_nibble` | structural | nzcv low nibble == `nzcv & 0xF`; upper bits independent |
| E | `prop_imm_vs_reg_differ_only_bit11` | differential | imm-form ⊕ reg-form == `1<<11` (o3) when imm5==Rm |
| F | `prop_rejects_out_of_range_immediates` | negative | **FAILS** — imm5∉0..=31 / nzcv∉0..=15 must return Err |

### Finding (bug)

`prop_rejects_out_of_range_immediates` **FAILS**: the immediate form masks `imm5` with `& 0x1F`
and `nzcv` with `& 0xF` without range validation, so e.g. `ccmn w0, #32, #16, eq` silently
encodes as `Ok(Word(0x3A400000))` (== `ccmn w0, #0, #0, eq`) and negative `imm5` such as `#-1`
becomes `#31`. Per ARM ARM both fields are unsigned with no wrap-around semantics. See
`pbt-out/bug_reports/encode_ccmp_ccmn_immediate_range_truncation.md`.

### Notes / observations (not bugs)

- **Register form `rm` is safe**: `parse_reg_num` already bounds register numbers to `0..=31`, so the unmasked `(rm << 16)` in the register form cannot corrupt bits above `[20:16]`. No masking gap there.
- **Property D characterizes the masking**: it documents the `& 0xF` behavior as-is; Property F is the *correctness* assertion that the masking should instead be a rejection.
