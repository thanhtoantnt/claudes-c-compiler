# PBT Coverage — `encode_cmp`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cmp`
**Suite:** `prop_encode_cmp_tests` (6 properties; 5 pass, 1 failing bug reproducer)

## Function under test

```rust
pub(crate) fn encode_cmp(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CMP Rn, op -> SUBS XZR, Rn, op
    let mut new_ops = vec![Operand::Reg("xzr".to_string())];
    new_ops.extend(operands.iter().cloned());
    let is_32 = if let Some(Operand::Reg(r)) = operands.first() {
        is_32bit_reg(r)
    } else { false };
    if is_32 { new_ops[0] = Operand::Reg("wzr".to_string()); }
    encode_add_sub(&new_ops, true, true)
}
```

## Properties

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| A | `prop_cmp_imm_structure` | structural — immediate form `SUBS ZR, Rn, #imm`: sf, op=1, S=1, opcode `10001`, sh=0, imm12 round-trips, Rn, Rd=31 | ✅ pass |
| B | `prop_cmp_reg_structure` | structural — register form `SUBS ZR, Rn, Rm`: opcode `01011`, shift=0, Rm, imm6=0, Rn, Rd=31 | ✅ pass |
| C | `prop_width_differs_only_bit31` | differential — `CMP Xn,op` ⊕ `CMP Wn,op` == `1<<31`; width driven solely by `is_32bit_reg(operands[0])` | ✅ pass |
| D | `prop_cmp_xor_cmn_is_bit30` | differential — `CMP` ⊕ `CMN` (sibling) == `1<<30`; subtract vs add | ✅ pass |
| E | `prop_rejects_unencodable_immediate` | negative contract — unshifted `imm` > 0xFFF (and not 0x1000-multiple) returns `Err`, not truncated | ✅ pass |
| F | `prop_rejects_large_imm_with_explicit_shift` | negative contract — `#imm, lsl #12` with imm > 0xFFF must be rejected | ❌ fail |

## Findings

### BUG-1: `encode_cmp` silently truncates an oversized `lsl #12` immediate

`CMP Rn, #imm, lsl #12` (forwarded verbatim by `encode_cmp` to `encode_add_sub`)
is masked into the 12-bit `imm12` field with `& 0xFFF` instead of being
range-validated. The comment in `encode_add_sub` even says the immediate "must
fit in 12 bits", but the code masks. e.g. `cmp w0, #4097, lsl #12` assembles as
`cmp w0, #1, lsl #12` (since `4097 & 0xFFF == 1`) instead of returning `Err`.
GAS/LLVM-MC reject this input. The plain unshifted path is correct (Property E).

- **Report:** `pbt-out/bug_reports/encode_cmp_lsl12_immediate_silent_truncation.md`
- **Root cause:** `encode_add_sub` explicit-shift immediate branch (`data_processing.rs`).
- **Suggested fix:** validate `imm_val > 0xFFF` before masking in the `explicit_shift` branch (then `encode_cmp` propagates the `Err`).

---

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

---

# PBT Coverage — `encode_cmn`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cmn`
**Suite:** `prop_encode_cmn_tests` (7 properties; 6 pass, 1 failing bug reproducer)

## Function under test

```rust
pub(crate) fn encode_cmn(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CMN Rn, op -> ADDS XZR, Rn, op
    let mut new_ops = vec![Operand::Reg("xzr".to_string())];
    new_ops.extend(operands.iter().cloned());
    let is_32 = if let Some(Operand::Reg(r)) = operands.first() {
        is_32bit_reg(r)
    } else { false };
    if is_32 { new_ops[0] = Operand::Reg("wzr".to_string()); }
    encode_add_sub(&new_ops, false, true)
}
```

## Properties

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| A | `prop_imm_form_structure` | structural — immediate form `ADDS ZR, Rn, #imm`: sf, op=0, S=1, opcode `10001` (+`[23]=0`), sh=0, imm12 round-trips, Rn, Rd=31 | ✅ pass |
| B | `prop_rd_always_zr` | invariant — destination register is always `XZR`/`WZR` (`Rd == 31`) for both forms and both widths | ✅ pass |
| C | `prop_cmn_vs_cmp_differs_only_op_bit` | differential — `CMN` ⊕ `CMP` == `1<<30` (add vs sub), both immediate and register forms | ✅ pass |
| D | `prop_sf_bit_is_bit31` | differential — `CMN Xn,#imm` ⊕ `CMN Wn,#imm` == `1<<31`; width driven solely by `is_32bit_reg(operands[0])` | ✅ pass |
| E | `prop_reg_form_structure` | structural — register form `ADDS ZR, Rn, Rm`: opcode `01011`, `[21]=0`, shift=0, Rm, imm6=0, Rn, Rd=31 | ✅ pass |
| F | `prop_rejects_unrepresentable_immediate` | negative contract — unshifted `imm` in `0x1001..=0x1FFF` (non-0x1000-multiple) returns `Err`, not truncated | ✅ pass |
| G | `prop_rejects_oversized_lsl12_immediate` | negative contract — `#imm, lsl #12` with imm > 0xFFF must be rejected | ❌ fail |

## Findings

### BUG-1: `encode_cmn` silently truncates an oversized `lsl #12` immediate

`CMN Rn, #imm, lsl #12` (forwarded verbatim by `encode_cmn` to `encode_add_sub`)
is masked into the 12-bit `imm12` field with `& 0xFFF` instead of being
range-validated. e.g. `cmn w0, #4097, lsl #12` assembles as
`cmn w0, #1, lsl #12` (since `4097 & 0xFFF == 1`) instead of returning `Err`.
GAS/LLVM-MC reject this input. This is the exact sibling of the `encode_cmp`
lsl-#12 bug — both reach the same explicit-shift branch in `encode_add_sub`.
The plain unshifted path is correct (Property F).

- **Report:** `pbt-out/bug_reports/encode_cmn_lsl12_immediate_silent_truncation.md`
- **Sibling:** `encode_cmp_lsl12_immediate_silent_truncation.md` (same root cause).
- **Root cause:** `encode_add_sub` explicit-shift immediate branch (`data_processing.rs`).
- **Suggested fix:** validate `imm_val > 0xFFF` before masking in the `explicit_shift` branch — a single fix closes the bug for `encode_cmn`, `encode_cmp`, and every other caller that forwards a trailing `lsl #12` into `encode_add_sub`.
