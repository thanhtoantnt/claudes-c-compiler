# PBT Coverage — `encode_bl`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_bl`
**Suite:** `prop_encode_bl_tests` (7 properties; 6 pass, 1 failing bug reproducer)

## Function under test

```rust
pub(crate) fn encode_bl(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (sym, addend) = get_symbol(operands, 0)?;
    // BL: 100101 imm26
    Ok(EncodeResult::WordWithReloc {
        word: 0b100101 << 26,                       // == 0x9400_0000
        reloc: Relocation { reloc_type: RelocType::Call26, symbol: sym, addend },
    })
}
```

## Properties

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| A | `prop_opcode_structure_and_imm26_zero` | structural — opcode `0b100101` in [31:26], imm26 [25:0] left zero, word == `0x94000000` | ✅ pass |
| B | `prop_bl_vs_branch_differ_only_bit31` | differential — BL ⊕ B == `1<<31`; BL has link bit set | ✅ pass |
| C | `prop_reloc_is_call26_with_symbol` | relocation contract — `Call26` type, symbol & addend mirror input across **all 8** `get_symbol`-accepted operand kinds | ✅ pass |
| D | `prop_word_is_operand_independent` | word invariance — instruction word is the fixed base `0x94000000` for every accepted operand kind | ✅ pass |
| E | `prop_rejects_non_symbol_and_empty_operands` | negative contract — 11 rejected operand kinds (Imm/Mem/Shift/Expr/…) **and** empty operand vector all return `Err` | ✅ pass |
| F | `prop_rejects_reg_cond_barrier_targets` | negative contract — `BL` must reject register, condition-code, and barrier tokens as call targets | ❌ fail |
| G | `prop_encoding_is_deterministic` | purity — repeated encoding yields bit-identical word + relocation | ✅ pass |

## Findings

### BUG-1: `encode_bl` silently accepts register/condition/barrier operands as call targets

`encode_bl` forwards `Reg`, `Cond`, and `Barrier` operands through `get_symbol`, so invalid inputs such as `bl x0`, `bl eq`, and `bl sy` return `Ok(WordWithReloc { word: 0x94000000, reloc: Call26 { symbol: <token>, addend: 0 } })` instead of `Err`.

This is the `BL` sibling of the `encode_branch` target-validation bug: register calls are `BLR`, and condition/barrier mnemonics are not labels unless parsed as label/symbol operands.

- **Report:** `pbt-out/bug_reports/encode_bl_silently_accepts_reg_cond_barrier_operands.md`
- **Suggested fix:** use a stricter branch-target helper for `B`/`BL` that accepts labels/symbols/symbol offsets/modifiers and rejects `Reg`, `Cond`, and `Barrier` token kinds.
