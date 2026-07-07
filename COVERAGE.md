Generated property-based tests for `compute_reverse_postorder` in `src/ir/analysis.rs`.

Properties covered:
- Empty CFG returns an empty RPO.
- RPO matches a reference DFS implementation on random graphs.
- RPO includes each reachable block exactly once.
- Duplicate successor edges do not change the traversal result.
- Non-empty graphs start with entry block `0`.

Verification:
- `cargo test compute_reverse_postorder -- --nocapture`

Generated property-based tests for `eval_binop_with_types` in `src/common/const_eval.rs`.

Properties covered:
- Shift operations select result width and signedness from the promoted left operand only.
- Non-shift operations select the left operand signedness when the left operand is wider.
- Non-shift operations select the right operand signedness when the right operand is wider.
- Non-shift operations use unsigned result semantics when either same-sized operand is unsigned.

Verification:
- `cargo test common::const_eval::tests -- --nocapture`

Generated property-based tests for `classify_cast_with_f128` in `src/backend/cast.rs`.

Properties covered:
- Non-native F128 mode reduces F128 casts to existing F64 cast classification semantics.
- Native F128-to-integer/ptr casts classify by destination signedness.
- Native integer/ptr-to-F128 casts classify by source signedness.
- Native F32/F64 <-> F128 edges use softfloat widen/narrow cast kinds.
- Identical source and destination types are noops in both native and non-native F128 modes.

Verification:
- `cargo test backend::cast::classify_cast_with_f128_focused_properties --lib`

## encode_lui (src/backend/riscv/assembler/encoder/base.rs)

5 property-based tests (proptest, 256 cases each), all PASS:
- `lui_imm_encodes_opcode_rd_and_imm_fields` — Reference oracle: U-format word
  has opcode LUI in bits[6:0], rd number in bits[11:7], and the low 20 bits of
  the immediate in bits[31:12]; cross-checked against `encode_u`.
- `lui_accepts_bare_number_destination` — GCC-style bare register numbers
  (Operand::Imm 0..=31) accepted as rd.
- `lui_symbol_emits_hi20_relocation` — `lui rd, %hi(sym)` yields WordWithReloc
  with zeroed imm field, RelocType::Hi20, addend 0, stripped symbol name.
- `lui_tprel_hi_symbol_emits_tprel_hi20_relocation` — `%tprel_hi(sym)` selects
  TprelHi20 instead of Hi20.
- `lui_rejects_invalid_operands` — Negative/error contract: non-Imm/non-Symbol
  2nd operand or missing 2nd operand returns "lui: invalid operands"; missing
  first operand and invalid register names also rejected.

Verification:
- `cargo test backend::riscv::assembler::encoder::base::pbt_encode_lui --lib`

## PBT coverage: `encode_shift_imm` (src/backend/riscv/assembler/encoder/base.rs)

Module `pbt_encode_shift_imm` (5 properties, all PASS, 256 cases each via proptest):

- `shift_imm_encodes_all_fields` — Reference oracle: every field of the I-format OP-IMM word (opcode/rd/funct3/rs1/imm[31:20]) is exactly determined by inputs.
- `shift_amt_is_masked_to_six_bits` — shamt is masked to `& 0x3F`; identical word for a shamt and its low-6-bits value, including negative immediates.
- `shift_imm_matches_encode_i_reference` — output equals `encode_i(OP_OP_IMM, rd, funct3, rs1, (funct6<<6)|(shamt&0x3F))`.
- `real_shift_mnemonics_decode_to_canonical_fields` — slli/srli/srai produce canonical RISC-V words (funct6 in bits[31:26], shamt in bits[25:20]).
- `shift_imm_rejects_invalid_operands` — Negative/error contract: missing operands, non-imm third operand, invalid register all rejected.

No bugs found. `encode_shift_imm` is a correct, thin wrapper over `encode_i`.

## PBT coverage: `encode_neon_movi` (src/backend/arm/assembler/encoder/neon.rs)

Module `movi_pbt_tests` (5 properties, all PASS, 256 cases each via proptest):

- `prop_rd_field_preserved` — Invariant: the Rd field (bits 4-0) of the encoded word always equals the source register number, across all valid arrangements.
- `prop_imm8_roundtrip` — Round-trip oracle: the 8-bit immediate reconstructs exactly from `abc` (bits 18-16) and `defgh` (bits 9-5) for every byte/halfword/word form (`.8b`/`.16b`/`.4h`/`.8h`/`.2s`/`.4s`).
- `prop_fixed_fields_per_arrangement` — ISA spec check: bit 31 is always 0, the Q bit (bit 30) selects the wide arrangement, and cmode (bits 15-12) matches the per-arrangement constant (`1110`/`1000`/`0000`).
- `prop_2d_byte_pattern_contract` — Differential oracle: `.2d` returns Ok iff every byte of the 64-bit immediate is `0x00` or `0xFF`; when Ok, the reconstructed imm8 equals the byte-mask and bits 31-28 are `0110` (Q=1, op=1).
- `prop_error_contracts` — Negative/error contract: missing immediate, unsupported arrangements (`.1d`/`.2h`/`.1q`), and invalid `.2s` LSL shift amounts (not in `{0,8,16,24}`) all return Err.

No bugs found. `encode_neon_movi` correctly implements the AArch64 SIMD modified-immediate encoding across all four arrangement classes.

Verification:
- `cargo test --lib movi_pbt_tests`

## PBT coverage: `encode_ccmp_ccmn` (src/backend/arm/assembler/encoder/compare_branch.rs)

Module `prop_ccmp_ccmn_tests` (5 properties, all PASS, 256 cases each via proptest):

- `prop_opcode_structure_and_fields` — Full reference oracle: fixed opcode bits (`[29]`=1, `[28:21]`=11010010, gaps `[10]`/`[4]`=0) are exact, and every operand-dependent field lands in its ISA-defined position — `sf`@[31], `op`@[30], `imm5`/`Rm`@[20:16], `cond`@[15:12], `o3`@[11], `Rn`@[9:5], `nzcv`@[3:0] — with `imm5`/`nzcv` masked to their field widths.
- `prop_ccmp_xor_ccmn_is_bit30` — Differential oracle: identical operands+cond yield CCMP and CCMN words that differ *only* in bit 30 (`ccmp ^ ccmn == 1<<30`).
- `prop_sf_bit_is_bit31` — Differential oracle: an `x`-register vs the matching `w`-register produce words that differ *only* in bit 31 (sf).
- `prop_nzcv_masked_to_nibble` — Masking invariant: the low nibble equals `nzcv & 0xF` and the rest of the word is identical whether `nzcv` or `nzcv & 0xF` is supplied (idempotent under masking).
- `prop_imm_vs_reg_differ_only_bit11` — Differential oracle: when `imm5 == Rm` register number, the immediate and register forms differ *only* in bit 11 (o3), since both place the same value in `[20:16]`.

No bugs found. `encode_ccmp_ccmn` correctly implements both the immediate (`#imm5`) and register (`Rm`) forms of the AArch64 conditional-compare instruction for CCMP and CCMN, in 32- and 64-bit register widths.

Verification:
- `cargo test --lib prop_ccmp_ccmn_tests`
