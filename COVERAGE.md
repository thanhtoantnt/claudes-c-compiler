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
