# PBT Coverage — `encode_mul`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Result:** 6/6 properties pass. **1 functional bug found** (silent SP→XZR aliasing) — see Bugs Found.

## What the function does
`MUL Rd, Rn, Rm` is encoded as `MADD Rd, Rn, Rm, XZR`:
```
word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (0b11111 << 10) | (rn << 5) | rd
```
i.e. ARMv8 MADD `sf 0 0 11011 000 Rm 0 Ra Rn Rd` with `Ra = XZR = 0b11111` and `o0 = 0`.
A leading `RegArrangement` first operand delegates to `encode_neon_mul`.

## Properties verified
1. `mul_xregs_field_placement` — spec-exact placement of every fixed + register field (reference).
2. `mul_sf_tracks_width` — `sf` (bit 31) reflects X vs. W (reference).
3. `mul_equals_madd_with_xzr` — bit-identical to `encode_madd(.., XZR)` for all widths/operands (differential oracle — the strongest check).
4. `mul_width_only_flips_sf_bit` — width affects only bit 31 (algebraic invariant).
5. `mul_rejects_too_few_operands` — <3 operands → `Err` (negative contract).
6. `mul_rejects_immediate_operand` — non-register Rm → `Err` (negative contract).

## Bugs Found

### BUG-1 (High): `encode_mul` silently accepts SP in any operand → encoded as multiply-by-zero
`mul x0, x1, sp` is accepted with `Ok(Word(...))` whose Rm field is 31 — i.e. it is silently
encoded as `mul x0, x1, xzr` (a multiply-by-zero). ARMv8 MADD/MUL has **no** SP-using variant;
field 31 is XZR, and SP in these operands is UNPREDICTABLE/unallocated. Root cause: the shared
`get_reg`→`parse_reg_num` helper maps `sp`/`wsp`→31 unconditionally (correct for SP-aware
ADD/SUB, wrong for every XZR-only data-processing instruction). Confirmed empirically via the
characterization test `mul_sp_in_rm_is_silently_accepted_as_xzr`; same aliasing hits SP in Rd
and Rn too, and sibling encoders (`encode_madd`/`encode_div`/`encode_logical` reg form/etc.).

- **Report:** `pbt-out/bug_reports/encode_mul_sp_operand_silently_accepted_as_xzr.md`
- **Repro:** `cargo test --lib backend::arm::assembler::encoder::data_processing::tests::mul_sp_in_rm_is_silently_accepted_as_xzr -- --nocapture`
- **Evidence:** `mul x0,x1,sp -> Ok(Word(2602531872))`, Rm field = 31 (XZR)
- **Suggested fix:** reject SP/WSP (and mixed widths) in `encode_mul` before encoding, or
  add a `get_reg_no_sp` helper used by all XZR-only data-processing encoders.

---

# PBT Coverage — `encode_madd`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_madd`
**Result:** 5/5 properties pass. **1 functional finding** (silent mixed-width operand acceptance) — see BUG-2.

## What the function does
Encodes ARMv8-A `MADD <Rd>,<Rn>,<Rm>,<Ra>` as
`sf 0 0 11011 000 Rm o0 Ra Rn Rd` with `o0 = 0`:
```
word = (sf << 31) | (0b0011011000 << 21) | (rm << 16) | (ra << 10) | (rn << 5) | rd
```
The `o0` bit (bit 15) is 0 for MADD and 1 for MSUB; `sf` is taken from operand 0 (Rd).

## Properties verified
1. `madd_field_placement` — spec-exact placement of every fixed field (bits 30:21 = `0b0011011000`,
   `o0 = 0`) and every register field (Rd/Rn/Rm/Ra) for both widths, Ra=31 (xzr) allowed.
2. `madd_msub_differ_only_in_o0` — `encode_madd ⊕ encode_msub == 1<<15` for all inputs
   (differential oracle against the sibling MSUB encoder).
3. `madd_width_only_flips_sf_bit` — X↔W swap of identical numbers changes only bit 31 (invariant).
4. `madd_rejects_too_few_operands` — `<4` operands → `Err` (negative contract).
5. `madd_ra_xzr_equals_mul` — `MADD Rd,Rn,Rm,XZR` is bit-identical to `encode_mul` (algebraic /
   reference oracle for the `MUL` alias).

## Bugs Found

### BUG-2 (Medium): `encode_madd` silently accepts mixed-width operands (sf taken from Rd only)
The encoder derives `sf` (bit 31) exclusively from operand 0 and never validates that all four
operands share the same width. `madd x0, w1, x2, x3` is accepted with `Ok(Word(0x9b028060)`)
whose `sf = 1` (64-bit) even though `Rn = w1` is a 32-bit register — an UNPREDICTABLE/
unallocated combination in AArch64. Same shape affects any data-processing encoder that calls
`get_reg(.., 0)` for width and ignores the `_` width of later operands (`encode_madd`,
`encode_msub`, `encode_mul`, `encode_div`, …). Confirmed by the characterization test
`madd_silently_accepts_mixed_width_operands`.

- **Repro:** `cargo test --lib madd_silently_accepts_mixed_width_operands -- --nocapture`
- **Evidence:** `madd x0,w1,x2,x3 -> Ok(Word(2600602656))`, `sf = 1, rn field = 1 (from w1)`
- **Suggested fix:** after resolving all four registers, assert their `is_64` flags agree and
  return `Err` on mismatch (a `get_reg_consistent` helper shared across this encoder family).

> Note: the SP→XZR silent aliasing documented in BUG-1 also applies to `encode_madd`'s operands
> (same `get_reg` root cause); it is not re-listed here.

---

# PBT Coverage — `encode_div`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_div`
**Result:** 5/5 properties pass. No *new* findings; two pre-existing findings (below) apply.

## What the function does
Encodes ARMv8-A `SDIV`/`UDIV <Rd>,<Rn>,<Rm>` as a data-processing (2 source) instruction:
```
word = (sf << 31) | (0b0011010110 << 21) | (rm << 16)
      | (0b00001 << 11) | (o1 << 10) | (rn << 5) | rd
```
i.e. `sf 0 S=0 11010110 Rm 00001 o1 Rn Rd`, where `o1=1` → SDIV (`opcode6=000011`),
`o1=0` → UDIV (`opcode6=000010`). `sf` is taken from operand 0 (Rd) only.
(Manually verified against the ARM ARM: `sdiv x0,x1,x2` → `0x9AC20C20`, `udiv x0,x1,x2` → `0x9AC20820`.)

## Properties verified
1. `div_sdiv_field_placement` — spec-exact placement of every fixed field (sf=1, reserved bit30=0,
   S=0, opcode bits 28:21=`11010110`, `opcode6=000011`, `o1=1`) and every register field (reference).
2. `div_udiv_field_placement` — same as #1 for UDIV (`opcode6=000010`, `o1=0`) (reference).
3. `div_sf_tracks_register_width` — `sf` (bit 31) reflects X vs. W (reference).
4. `div_sdiv_udiv_differ_only_in_o1` — for identical operands `SDIV ⊕ UDIV == 1<<10` exactly
   (differential / algebraic oracle between the two halves of `encode_div`).
5. `div_register_fields_isolated` — Rm affects only bits 20:16, Rn only bits 9:5, Rd only bits 4:0
   (no inter-field aliasing/truncation).

## Bugs Found
None new. The two findings already documented for the sibling encoders also apply to `encode_div`
(both stem from the shared `get_reg`→`parse_reg_num`/sf-from-Rd-only path) and are **not** re-filed:
- **BUG-1** — SP/WSP in any operand is silently aliased to XZR (field 31), e.g. `sdiv x0,x1,sp`
  encodes as `sdiv x0,x1,xzr` with no error. See `pbt-out/bug_reports/encode_mul_sp_operand_silently_accepted_as_xzr.md`.
- **BUG-2** — mixed-width operands (e.g. `sdiv x0,w1,x2`) are silently accepted; `sf` is derived
  solely from Rd. See the `encode_madd` BUG-2 note above.

---

# PBT Coverage — `encode_smull`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smull`
**Result:** 5/5 properties pass. No *new* findings; one pre-existing finding (below) applies.

## What the function does
Encodes the `SMULL <Xd>,<Wn>,<Wm>` alias as `SMADDL <Xd>,<Wn>,<Wm>,<XZR>`:
```
word = (1u32 << 31) | (0b0011011001 << 21) | (rm << 16) | (0b011111 << 10) | (rn << 5) | rd
```
i.e. ARMv8-A `1 00 11011 001 Rm 0 Ra Rn Rd` with `sf = 1` (SMULL always yields a 64-bit
result), `o0 = 0` (bit 15), and `Ra = XZR = 0b11111`. The `is_64` flags returned by
`get_reg` are deliberately discarded — `sf` is hardcoded to `1`, which is correct for
SMULL/SMADDL (a 32×32→64 multiply). Register numbers are range-checked by `parse_reg_num`
(0–31), so no 5-bit field can overflow/truncate.
(Manually verified vs. the ARM ARM: `smull x0,w1,w2` → `0x9B227C20`, fixed base `0x9B207C00`.)

## Properties verified
1. `smull_matches_armv8_reference` — every valid register triple (0–31 each) encodes to
   exactly the spec SMADDL-with-XZR word `0x9B207C00 | (Rm<<16) | (Rn<<5) | Rd` (differential
   oracle — strongest check).
2. `smull_opcode_bits_constant` — all non-register bits are the constant `0x9B207C00`
   regardless of register choice; spot-checks each fixed field (sf=1, bits30:29=00,
   opcode=11011, class=001, o0=0, Ra=11111) against the spec.
3. `smull_register_fields_isolated` — Rd→bits 4:0, Rn→bits 9:5, Rm→bits 20:16 are each placed
   in their own 5-bit slot with no cross-field aliasing.
4. `smull_deterministic` — identical operands always yield the identical word.
5. `smull_rejects_invalid_operands` — <3 operands, a non-register operand, and an
   out-of-range register number (`x32`..) all return `Err` (negative contract; no silent
   truncation/wrapping).

## Bugs Found
None new. The SP/WSP→XZR silent aliasing already documented in **BUG-1** also applies to
`encode_smull` (same shared `get_reg`→`parse_reg_num` path: `sp`/`wsp` map to field 31 =
XZR/WZR). None of SMULL's operands (`Xd`, `Wn`, `Wm`) may be SP, so e.g. `smull x0, wsp, w2`
is silently encoded as `smull x0, wzr, w2`. Not re-filed — see
`pbt-out/bug_reports/encode_mul_sp_operand_silently_accepted_as_xzr.md`. (BUG-2 / mixed-width
is *not* applicable: SMULL is by definition a mixed-width instruction — 32-bit sources, 64-bit
destination — and `sf` is correctly hardcoded to 1.)

## encode_mneg — data_processing.rs (MNEG Xd, Xn, Xm → MSUB Xd, Xn, Xm, XZR)

**Result: PASS, no finding.** 5 proptest properties added to the existing
`mod tests` block in `data_processing.rs`:

1. `mneg_field_placement` — full-word equality vs an independently constructed
   spec word (`sf 00 11011 000 Rm 1 11111 Rn Rd`), for both W and X destination
   widths, plus register-field extraction (Rm=20:16, Rn=9:5, Rd=4:0).
2. `mneg_register_fields_isolated` — perturbing each register changes only its
   own 5-bit field; no register bleeds into another field or the opcode.
3. `mneg_constant_fields_invariant` — across all register combos the constant
   base is `0x9B00FC00` (64-bit) / `0x1B00FC00` (32-bit); explicitly pins
   **o1 (bit 15) = 1** (MSUB, not MADD), Ra=11111 (XZR), opcode=11011, o0=000.
4. `mneg_rejects_invalid_operands` — negative contract: <3 operands, a
   non-register operand anywhere, or an out-of-range register (>31) → `Err`
   (no silent truncation, no panic).
5. `mneg_alias_equals_msub_with_xzr` — architectural alias differential:
   `MNEG Xd,Xn,Xm` is bit-identical to `MSUB Xd,Xn,Xm,XZR`; both select MSUB
   (o1=1). Confirms `encode_mneg` and `encode_msub` agree.

`encode_mneg` is a faithful encoding of the ARMv8 MNEG/MSUB alias. No SP/XZR
operand validation gap exists here (unlike MUL/SMULL) because all three operands
flow through `get_reg`, which rejects non-register/out-of-range operands; the
only effect of passing `sp`/`xzr` is the architecturally-valid encoding of
register 31 = XZR.

---

# PBT Coverage — `encode_cond_branch`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cond_branch`
**Result:** 7/7 properties pass. **No bug found.** Encoding is spec-correct.

## What the function does
Encodes the AArch64 conditional branch `B.<cond> <target>` (ARM ARM C5.6.6):
```
word  = (0b01010100 << 24) | encode_cond(cond)   // imm19 [23:5] left 0 for the linker
result = WordWithReloc { word, reloc: CondBr19(symbol, addend) }
```
i.e. `0101 0100 | imm19 | 0[4] | cond[3:0]`. `encode_cond` lowercases its input, so
condition codes are matched case-insensitively; `get_symbol` resolves the branch target
into a `(symbol, addend)` pair forwarded into a `CondBr19` relocation. The `imm19` offset
is intentionally left zero — the linker patches it.

## Properties verified (module `prop_encode_cond_branch_tests`)
1. `prop_opcode_structure_and_fields` — opcode byte `0x54` in [31:24], imm19 field [23:5]
   zero, o0 bit [4] zero, cond in [3:0]; word == `OPCODE | cond` exactly (reference oracle).
2. `prop_cond_field_matches_table` — every name in the canonical cond table (incl. `al`/`nv`
   edges) round-trips to its 4-bit value.
3. `prop_aliases_encode_identically` — carry aliases `cs`/`hs` and `cc`/`lo` are bit-identical
   (differential).
4. `prop_reloc_is_condbr19_with_symbol` — `CondBr19` relocation with exact symbol/addend
   forwarding across every operand kind `get_symbol` accepts.
5. `prop_unknown_condition_rejected` — classifier/negative contract: arbitrary lowercase tokens
   are accepted iff present in the cond table; unknown → `Err`.
6. `prop_rejects_non_symbol_operands` — negative contract: every operand kind `get_symbol`
   rejects (Imm/Mem*/Shift/Extend/Expr/RegArrangement/RegLane/RegList…) → `Err`; no silent
   encoding of an invalid branch target.
7. **`prop_condition_is_case_insensitive`** (new) — locks the case-folding contract: `EQ`/`eq`/`Eq`
   all encode to a bit-identical word. Closes the one gap the lowercase-only generators above
   could not reach (`encode_cond`'s `to_lowercase()` + the mnemonic dispatcher's pre-lowering).

## Bugs Found
None. The core encoding, condition mapping (incl. aliases and `al`/`nv`), imm19-zero linker
contract, and `CondBr19` symbol/addend forwarding are all correct. The one non-trivial behavior
beyond the structural oracle — case-insensitive condition matching — is intended
(`to_lowercase()` is explicit, and consistent with the `b.<cond>` dispatcher which lowers the
whole mnemonic first) and is now pinned by Property 7 rather than left implicit.

---

# PBT Coverage — `encode_branch`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_branch`
**Result:** 8/8 properties pass. **1 functional bug found** (silent acceptance of
`Reg`/`Cond`/`Barrier` operands as branch targets) — see BUG below.

## What the function does
Encodes the AArch64 unconditional branch `B <target>` (ARM ARM C5.6.5):
```
word   = 0b000101 << 26          // == 0x1400_0000; imm26 [25:0] left 0 for the linker
result = WordWithReloc { word, reloc: Jump26 { symbol, addend } }
```
`get_symbol(operands, 0)` resolves the branch target into a `(symbol, addend)` pair
forwarded into a `Jump26` relocation; the `imm26` branch-offset field is intentionally
left zero and patched by the linker. There is no immediate operand to validate — the
encoder accepts only a symbol-like operand and otherwise returns `Err`.

## Properties verified (module `prop_encode_branch_tests`)
1. `prop_opcode_structure_and_imm26_zero` — opcode `0b000101` in [31:26], imm26 [25:0] zero;
   word is exactly `0x1400_0000` for a `Symbol` target (reference oracle).
2. `prop_branch_vs_bl_differ_only_bit31` — `encode_branch ⊕ encode_bl == 1<<31` exactly
   (differential oracle against the BL sibling — the link bit).
3. `prop_reloc_is_jump26_primary_forms` — `Jump26` relocation with exact symbol/addend
   forwarding for the `Symbol` and `SymbolOffset` forms.
4. `prop_symbol_forwarding_all_accepted_kinds` — `Jump26` relocation with exact symbol/addend
   forwarding across all 8 operand kinds `get_symbol` accepts (Symbol/Label/SymbolOffset/
   Modifier/ModifierOffset/Reg/Cond/Barrier).
5. `prop_rejects_non_symbol_operands` — negative contract: every operand kind `get_symbol`
   rejects (Imm/Mem*/Shift/Extend/Expr/RegArrangement/RegLane) → `Err`; no silent encoding
   of an invalid branch target.
6. **`prop_word_is_operand_independent`** (new) — the instruction word is the constant
   `0x1400_0000` for EVERY accepted operand kind; the operand influences only the
   relocation. Generalises Property 1 (which only checks the `Symbol` form) and complements
   Property 4 (which only checks the relocation across kinds).
7. **`prop_empty_operands_rejected`** (new) — arity / negative contract: a branch with no
   target operand → `Err`. Property 5 covers the wrong *type* of operand; this closes the
   *missing*-operand edge (`get_symbol` reads `operands[0]` unconditionally).
8. **`prop_encoding_is_deterministic`** (new) — purity: encoding the same operand repeatedly
   yields bit-identical word AND relocation (symbol + addend).

## Bugs Found

### BUG: `encode_branch` silently accepts `Reg`/`Cond`/`Barrier` operands as branch targets
`b x0`, `b wzr`, `b eq`, `b sy` are silently accepted as `Jump26` relocations against
spurious symbols named `"x0"`/`"wzr"`/`"eq"`/`"sy"` (word `0x1400_0000`) instead of being
rejected. The `B` instruction takes only a label/offset (register-branch is `BR`); the
acceptance stems from `get_symbol` forwarding `Reg`/`Cond`/`Barrier` tokens as relocation
symbols. Harm: a confusing unresolved-symbol link error pointing at a register name, or — if a
same-named label exists — a silent mis-targeted branch with no diagnostic. Confirmed by the
passing characterization `prop_symbol_forwarding_all_accepted_kinds`.

- **Report:** `pbt-out/bug_reports/encode_branch_silently_accepts_reg_cond_barrier_operands.md`
- **Repro:** `cargo test --lib prop_encode_branch_tests::prop_symbol_forwarding_all_accepted_kinds -- --nocapture`
- **Evidence:** `b x0 -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "x0", addend: 0 } })`
- **Suggested fix:** add a `get_symbol_strict` (Symbol/Label/SymbolOffset/Modifier only) used by
  the direct-branch encoders; scope the label-collision workaround to the parser.

The encoding itself is otherwise spec-correct: the fixed opcode `0b000101 << 26` is right, the
`imm26` linker-reserved field is left zero, and the `Jump26` relocation correctly forwards
`(symbol, addend)` for genuine symbol operands. There is no immediate operand, so the
truncation/range concern that applies to TBZ/CBZ/CCMP does not arise here.
