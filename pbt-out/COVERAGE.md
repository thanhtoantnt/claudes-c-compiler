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

---

# PBT Coverage — `encode_cbz`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cbz(operands, is_nz)`
**Result:** 6/6 properties pass. **1 functional bug found** (SP/WSP silently aliased to XZR/WZR) — see Bugs Found.

## What the function does
Encodes ARMv8-A `CBZ`/`CBNZ <Rt>, <label>` as `sf 011010 op imm19 Rt` and
emits a `CondBr19` relocation for the linker to fill the imm19 offset:
```
word = (sf << 31) | (0b011010 << 25) | (op << 24) | rt   // op=0 CBZ, op=1 CBNZ
```

## Properties verified
1. `prop_opcode_structure_and_fields` — spec-exact placement of every fixed + register field (reference oracle); reconstructs the whole word.
2. `prop_cbz_xor_cbnz_is_bit24` — `CBZ ⊕ CBNZ == 1<<24` for all inputs (differential).
3. `prop_sf_bit_is_bit31` — X↔W swap of identical numbers changes only bit 31 (differential invariant).
4. `prop_reloc_is_condbr19_with_symbol` — result carries a CondBr19 relocation whose symbol/addend mirror the operand (reference, across every `get_symbol`-accepted kind).
5. `prop_rejects_invalid_operands` — bad operand types / too-few operands → `Err` (negative contract).
6. `prop_sp_silently_aliased_to_xzr` — characterization pinning BUG-1 (see below).

## Bugs Found

### BUG-1 (High): `encode_cbz`/`encode_cbnz` silently accept SP/WSP, aliased to XZR/WZR
`cbz sp, <target>` encodes bit-identically to `cbz xzr, <target>` (Rt=31=XZR,
sf=1); `cbz wsp` ≡ `cbz wzr` (sf=0); same for CBNZ. Per the ARM ARM there is
**no SP-using form** of CBZ/CBNZ, so these must be rejected. Instead a branch
intended to test the stack pointer is silently mis-assembled into a branch on
the *zero* register (CBZ on XZR is unconditionally taken). Root cause: the
shared `get_reg`→`parse_reg_num` maps `sp`/`wsp`→31 (correct for SP-aware ops,
wrong for every XZR-only instruction) and `encode_cbz` does no width/SP check.
Same defect family as `encode_mul`/`encode_div`/`encode_logical`.

- **Report:** `pbt-out/bug_reports/encode_cbz_sp_operand_silently_accepted_as_xzr.md`
- **Repro (property):** `cargo test --lib prop_encode_cbz_tests::prop_sp_silently_aliased_to_xzr -- --nocapture`
- **Evidence:** `cbz sp,target -> Ok(.. 0xB400001F ..)` (== `cbz xzr,target`); Rt field = 31.
- **Suggested fix:** add a `get_reg_no_sp` helper and route `encode_cbz` (and
  the rest of the XZR-only encoders) through it; then flip the property's
  `is_ok()` assertions to `is_err()`.

---


---

# PBT Coverage — `encode_adc`

**File:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_adc`
**Framework:** `proptest` (already a dev-dependency).

## Status: already comprehensively covered — all tests PASS

The target function already has a complete property-based test suite in the
file's `#[cfg(test)] mod tests` block (around lines 2660–2770). No new tests
were needed; an attempted addition only produced duplicate definitions and was
reverted. The existing suite was re-run and passes (7/7 ADC/SBC tests green).

## Existing properties (all passing)

| # | Property | Oracle type |
|---|----------|-------------|
| 1 | `adc_field_placement` — sf=1, op(bit30)=0, fixed opcode bits 28:21 = `11010000`, reserved bits 15:10 = 0, and Rm/Rn/Rd placement | Reference (spec field layout) |
| 2 | `adc_s_bit_tracks_set_flags` — S bit (bit 29) == `set_flags` (ADC vs ADCS) | Algebraic |
| 3 | `adc_sf_tracks_register_width` — sf (bit 31) tracks W→0 / X→1 | Algebraic |
| 4 | `adc_vs_sbc_op_bit` — differential: ADC op=0, SBC op=1 for identical operands | Differential |
| 5 | `adc_rejects_bad_operand_arities` — <3 operands, or non-register (immediate) in an operand slot → `Err` | Negative contract |

Plus a sibling `sbc_known_constant_encoding` independent hand-decoded
reference oracle for the closely-related `encode_sbc`.

## Finding (functional, reported separately)

A genuine correctness gap was found and is documented in
`pbt-out/bug_reports/encode_adc_silent_mixed_width.md`: **mismatched operand
widths are silently encoded** (e.g. `adc x0, w1, x2` → `Ok(0x9A020020)` = `adc
x0, x1, x2`), because `is_64` is taken only from `Rd` and the source widths are
discarded. This is architecturally UNDEF and is rejected by GNU `as`/LLVM. It
is a codebase-wide pattern, not specific to `encode_adc`.

A regression property for this gap is proposed in the bug report but is **not**
checked into the test file because it would fail today.
