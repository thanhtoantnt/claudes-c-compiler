# Bug Report — `encode_adc` silently accepts mismatched operand widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_adc`
**Severity:** Low–Medium (correctness / silent mis-assembly)
**Status:** Confirmed by failing property `adc_rejects_mixed_width_operands`.

## Summary

`encode_adc` derives the instruction width (`sf`, bit 31) **only** from the
destination operand `Rd` (operand 0). The widths of the source operands `Rn`
and `Rm` are read with `let (rn, _) = get_reg(...)` / `let (rm, _) = ...` — the
`is_64` flag is discarded. As a result, an instruction with **mixed X/W operand
widths is silently encoded** using `Rd`'s width, instead of being rejected as a
malformed instruction.

## Reproduction

```text
adc x0, w1, x2          // programmer wrote 32-bit source w1
  => encode_adc returns Ok(Word(0x9A020020))
```

`0x9A020020` decodes as `adc x0, x1, x2` — i.e. the `w1` was silently
**promoted to `x1`** with no diagnostic.

## Expected behaviour (ARMv8 ARM, C4.1.4 / C6.2.4)

All register operands of an `ADC`/`ADCS` instruction must share the same width
(`sf`). A width mismatch is architecturally **UNDEFINED** and assemblers are
expected to reject it with an "operand size mismatch" diagnostic (confirmed
behaviour of GNU `as` and LLVM `llvm-mc` for `adc x0, w1, x2`).

## Why it matters

A program containing a typo or macro-generated `adc x0, w1, x2` assembles
without error but produces a 64-bit operation that reads the **full 64-bit**
`x1` register rather than the intended 32-bit `w1` — a silent semantic
divergence with no compile-time signal.

## Scope note

This is a **codebase-wide pattern**, not unique to `encode_adc`. The same
discard-and-ignore idiom appears in `encode_add_sub`, `encode_sbc`,
`encode_logical`, `encode_mul`, `encode_madd`, `encode_div`, etc. A fix should
ideally be applied at the helper level (e.g. have `get_reg`/a wrapper validate
that all operands share the destination's width, or add an explicit width check
in each three-operand encoder).

## Regression property

Failing property: `adc_rejects_mixed_width_operands`

```rust
prop_assert!(encode_adc(&[xreg(rd), wreg(rn), xreg(rm)], false).is_err());
```

This currently fails because the encoder returns `Ok`.
