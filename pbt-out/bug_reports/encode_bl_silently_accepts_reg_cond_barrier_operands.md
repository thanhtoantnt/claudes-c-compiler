# Bug Report: `encode_bl` silently accepts register/condition/barrier operands as call targets

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_bl`

## Summary

`encode_bl` calls the shared `get_symbol` helper for its target operand. `get_symbol` accepts `Reg`, `Cond`, and `Barrier` operands by forwarding their text as a relocation symbol. For `BL`, those token kinds are not valid call targets: register calls use `BLR`, and condition/barrier mnemonics are not labels unless parsed as label/symbol operands.

## Reproduction

Failing property: `prop_rejects_reg_cond_barrier_targets`

Minimal examples:

```text
bl x0
bl eq
bl sy
```

Actual behavior: each returns `Ok(WordWithReloc { word: 0x94000000, reloc: Call26 { symbol: <token>, addend: 0 } })`.

Expected behavior: reject these operands at assembler level with `Err`.

## Impact

Invalid `BL` source is accepted and emitted as a relocation against names like `x0`, `eq`, or `sy`. This either becomes a confusing unresolved-symbol link error or, if a same-named label exists, a silently mis-targeted call.

## Suggested fix

Use a stricter branch-target helper for `B`/`BL` that accepts only labels, symbols, symbol offsets, and symbol modifiers, and rejects parser token kinds that are instruction operands in this context (`Reg`, `Cond`, `Barrier`).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/12
