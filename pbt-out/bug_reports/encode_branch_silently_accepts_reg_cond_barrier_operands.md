# BUG: `encode_branch` silently accepts `Reg`/`Cond`/`Barrier` operands as branch targets

**Function:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_branch`
**Severity:** Medium (silent acceptance of invalid branch forms → spurious / mis-targeted relocation)
**Found by:** `prop_encode_branch_tests::prop_symbol_forwarding_all_accepted_kinds` (passing
characterization) + evidence probe.

## Summary

`encode_branch` (the `B <target>` unconditional-branch encoder) resolves its single target via
the shared helper `get_symbol(operands, 0)`. That helper accepts not only `Symbol`/`Label`/
`SymbolOffset`/`Modifier{}`/`ModifierOffset{}` but also `Operand::Reg`, `Operand::Cond`, and
`Operand::Barrier`, forwarding the register/condition/barrier token verbatim as the relocation
**symbol**. As a result the following *invalid* AArch64 inputs are silently accepted as
`Jump26` relocations against spurious symbol names instead of being rejected:

```
b x0  -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "x0",  addend: 0 } })
b wzr -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "wzr", addend: 0 } })
b eq  -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "eq",  addend: 0 } })
b sy  -> Ok(WordWithReloc { word: 0x1400_0000, reloc: Jump26 { symbol: "sy",  addend: 0 } })
```

(`b` with no target is correctly rejected — see `prop_empty_operands_rejected`; the defect is
specifically the silent acceptance of the `Reg`/`Cond`/`Barrier` operand forms.)

## Why this is a defect

The AArch64 `B` instruction takes **only** a branch label/offset as its operand (ARM ARM C5.6.5).
Branching *to a register* is a different instruction, `BR <Xn>`, handled by `encode_br`. So
`b x0`, `b wzr`, `b eq`, `b sy` are not valid `B` operands. Accepting them has two concrete harms:

1. **Misleading link-time failure.** The relocation is emitted against a symbol named `"x0"`,
   `"eq"`, etc. — symbols that almost never exist as labels — so the user gets an
   unresolved-symbol error pointing at a register/condition name, rather than an
   assembler-level "this is not a valid branch target" message that would point them at `br`.
2. **Silent mis-targeting if a same-named symbol exists.** If a label `x0:` (or `eq:`, `sy:`)
   *does* exist in the translation unit, `b x0` silently becomes a branch to that label rather
   than an error that the intended operand was a register. The encoding is then "valid" but the
   programmer's intent (register branch) is lost with no diagnostic.

The `get_symbol` comment claims these branches are a parser-misclassification workaround for
symbol names colliding with register/condition/barrier names. That justification does not hold
for `encode_branch`: a `B` target is *always* a label, so a `Reg`/`Cond`/`Barrier` operand here
can only have come from (a) a typo for `br`/`b.<cond>`/`dsb`, or (b) using a reserved name as a
label — both of which an assembler should diagnose, not silently encode as a relocation.

## Repro

```bash
cargo test --lib backend::arm::assembler::encoder::compare_branch::prop_encode_branch_tests::prop_symbol_forwarding_all_accepted_kinds -- --nocapture
```
That property passes *because* of the defect — it asserts `b x0`/`b eq`/`b sy` each yield a
`Jump26` relocation carrying the register/condition/barrier name as the symbol. The evidence
lines above were captured from a temporary probe calling `encode_branch` directly.

## Suggested fix

In `encode_branch` (and the other direct branch encoders `encode_bl`, `encode_cond_branch`,
`encode_cbz`, `encode_tbz`), reject operand kinds that are not genuine branch targets before
constructing the relocation — e.g. a `get_symbol_strict` that accepts only `Symbol`/`Label`/
`SymbolOffset`/`Modifier{}`/`ModifierOffset{}` and returns `Err` for `Reg`/`Cond`/`Barrier`/
`Imm`/`Mem*`/etc. The label-collision workaround, if still needed, should be scoped to the
parser (so a label `x0:` is classified as `Symbol`, not `Reg`) rather than papered over at
encode time for every branch instruction.

## Root cause

Shared helper `get_symbol` (`encoder/mod.rs`) accepts `Operand::Reg`/`Operand::Cond`/
`Operand::Barrier` and forwards their inner string as the relocation symbol. `encode_branch`
calls it unconditionally, so all three operand kinds are silently treated as branch targets.
The same root cause affects `encode_bl`, `encode_cond_branch`, `encode_cbz`, `encode_tbz`, but
this report is scoped to `encode_branch` per per-function filing.
