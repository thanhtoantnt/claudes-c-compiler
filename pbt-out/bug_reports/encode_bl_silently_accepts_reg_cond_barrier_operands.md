# Bug Report: `encode_bl` silently accepts register/condition/barrier operands as call targets

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_bl`
**Severity:** Medium

## Summary

`encode_bl` calls `get_symbol` helper which accepts `Reg`, `Cond`, `Barrier` operands, forwarding text as relocation symbol. For `BL`, these token kinds are invalid call targets — register calls use `BLR`, conditions/barriers are not labels.

## Root Cause

`get_symbol` returns `Ok(WordWithReloc)` for any operand kind text. `BL` expects only label/symbol targets.

## Reproduction

**Input:** `bl x0`

**Expected:** `Err` — invalid operand for BL

**Actual:** `Ok(WordWithReloc { word: 0x94000000, reloc: Call26 { symbol: "x0", addend: 0 } })`

**Other failing inputs:** `bl eq`, `bl sy`

## Impact

Invalid `BL` accepted and emitted as relocation against `x0`, `eq`, or `sy`. If same-named label exists, silently mis-targeted call. If not, confusing unresolved-symbol error.

## Suggested Fix

Use stricter branch-target helper for `B`/`BL` accepting only labels/symbols/symbol offsets/modifiers, rejecting `Reg`/`Cond`/`Barrier` in this context:

```rust
let target = match &operands[0] {
    Operand::Label(name) | Operand::Symbol(name) => name.clone(),
    Operand::SymbolOffset { base, offset } => format!("{}+{}", base, offset),
    _ => return Err("BL target must be a label or symbol".to_string()),
};
```

## Regression Property

Failing property: `prop_rejects_reg_cond_barrier_targets`

```rust
prop_assert!(encode_bl(&[Operand::Reg("x0".into())]).is_err());
prop_assert!(encode_bl(&[Operand::Cond("eq".into())]).is_err());
prop_assert!(encode_bl(&[Operand::Barrier("sy".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/12