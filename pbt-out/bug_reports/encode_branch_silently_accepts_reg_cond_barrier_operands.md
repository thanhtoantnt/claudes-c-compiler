# Bug Report: `encode_branch` silently accepts `Reg`/`Cond`/`Barrier` operands as branch targets

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_branch`
**Severity:** Medium

## Summary

`encode_branch` (the `B <target>` unconditional-branch encoder) resolves its single target via the shared helper `get_symbol(operands, 0)`. That helper accepts not only `Symbol`/`Label`/`SymbolOffset`/`Modifier{}`/`ModifierOffset{}` but also `Operand::Reg`, `Operand::Cond`, and `Operand::Barrier`, forwarding the register/condition/barrier token verbatim as the relocation **symbol**. As a result, invalid AArch64 inputs are silently accepted as `Jump26` relocations.

Example invalid inputs that are silently accepted:
```
b x0  -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "x0", addend: 0 } })
b wzr -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "wzr", addend: 0 } })
b eq  -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "eq", addend: 0 } })
```

## Root Cause

Shared helper `get_symbol` (`encoder/mod.rs`) accepts `Operand::Reg`/`Operand::Cond`/`Operand::Barrier` and forwards their inner string as the relocation symbol. `encode_branch` calls it unconditionally, so all three operand kinds are silently treated as branch targets.

## Reproduction

**Input:** `b x0`

**Expected:** `Err` — operand must be a branch target (symbol/label), not a register

**Actual:** `Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "x0", addend: 0 } })`

**Minimal failing input:** `encode_branch(&[Operand::Reg("x0".into())])`

## Impact

Two concrete harms:
1. **Misleading link-time failure**: relocation emitted against symbol `"x0"`/`"eq"` etc. that rarely exist as labels, giving an unresolved-symbol error at a register/condition name rather than an assembler-level diagnostic pointing to `br`
2. **Silent mis-targeting if same-named symbol exists**: if a label `x0:` exists, `b x0` silently branches there instead of diagnosing the register operand

## Suggested Fix

Reject operand kinds that are not genuine branch targets before constructing the relocation:

```rust
fn get_symbol_strict(operands: &[Operand]) -> Result<SymbolWithOffset, String> {
    match operands.get(0) {
        Some(Operand::Label(_) | Operand::Symbol(_) | Operand::SymbolOffset(_) |
                Operand::Modifier { .. } | Operand::ModifierOffset { .. }) => Ok(...),
        Some(Operand::Reg(_) | Operand::Cond(_) | Operand::Barrier(_) | Operand::Imm(_) | Operand::Mem { .. }) => {
            Err("branch target must be a symbol or label, not a register/condition/barrier".into())
        }
        _ => Err("branch target expected".into()),
    }
}
```

## Regression Property

Failing property: `prop_symbol_forwarding_all_accepted_kinds`

```rust
prop_assert!(!matches!(encode_branch(&[Operand::Reg("x0".into())]), Ok(EncodeResult::Jump26(_))));
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/19