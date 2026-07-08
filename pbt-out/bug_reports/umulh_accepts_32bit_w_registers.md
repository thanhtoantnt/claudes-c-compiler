# BUG: `encode_umulh` accepts 32-bit (W) register operands without validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` — `encode_umulh`
**Severity:** Medium (silent mis-assembly / architecturally UNDEF output)

## Summary

`encode_umulh` calls `get_reg(operands, i)` for each operand, which returns
`(reg_num, is_64)`, but it **discards the `is_64` flag** and never validates
that the operands are 64-bit (`X`) registers. As a result, an input such as

```
umulh w0, w1, w2
```

is accepted and silently re-encoded as a 64-bit instruction (`sf = 1`), instead
of being rejected.

## Specification

Per the ARMv8 Architecture Reference Manual, **UMULH** (Unsigned Multiply High)
is defined **only** as

```
UMULH Xd, Xn, Xm        ; Xd, Xn, Xm must be 64-bit (X) registers
```

There is **no 32-bit (W) form**. The encoding space `sf=0` for op31=110 is
UNDEFINED. Therefore a correct assembler must reject any W-register operand
with an error.

## Reproduction

```rust
// umulh w0, w1, w2  -> should be Err, currently returns Ok(0x9be27c20)
let ops = vec![Operand::Reg("w0".into()), Operand::Reg("w1".into()),
               Operand::Reg("w2".into())];
encode_umulh(&ops)   // Ok(Word(0x9be27c20))  -- BUG
```

The produced word `0x9be27c20` is identical to `umulh x0, x1, x2`, i.e. the
assembler silently "promotes" 32-bit register names to their 64-bit encoding.

Minimal failing input found by `proptest`: `rd = 0, rn = 0, rm = 0`
(`umulh w0, w0, w0`) yields `Ok(0x9bc07c00)` instead of `Err`.

## Root cause

```rust
pub(crate) fn encode_umulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    // UMULH: 1 00 11011 1 10 Rm 0 11111 Rn Rd
    let word = (1u32 << 31) | (0b0011011110 << 21) | (rm << 16)
             | (0b011111 << 10) | (rn << 5) | rd;   // sf hardcoded to 1, no width check
    Ok(EncodeResult::Word(word))
}
```

The hardcoded `sf = 1` is *correct* (UMULH is inherently 64-bit), and the fixed
opcode bits (`11011`, op31=`110`, `o0=0`, Ra=`11111`) are all verified correct
by the passing properties below. The defect is solely the **absence of a width
check** on the operands.

## What is verified correct (passing properties)

The encoding itself is bit-exact against an independent literal-spec
reconstruction (`umulh_ref`):

- `umulh_fixed_fields` — sf=1, bits 30:29=00, class opcode 11011, op31=110,
  o0=0, Ra=XZR (11111) all pinned correctly.
- `umulh_register_fields_match_reference` — Rd/Rn/Rm land in bits 4:0 / 9:5 /
  20:16, and the whole word equals `0x9bc07c00 | (rm<<16) | (rn<<5) | rd`.
- `umulh_missing_operand_errors` — <3 operands correctly returns `Err`.
- `smulh_vs_umulh_only_sign_bit_differs` — UMULH/SMULH differ only in bit 23
  (op31 unsigned/signed selector).

## Related

Sibling of `smulh_accepts_32bit_w_registers.md`, which already lists UMULH in
its scope table. The same discard-`is_64` / no-width-validation pattern affects
the entire long/high-multiply family (`SMULH`, `UMULH`, `SMULL`, `UMULL`,
`SMADDL`, `UMADDL`).

## Suggested fix

Require `is_64 == true` for all three operands, returning `Err` otherwise:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !(rd64 && rn64 && rm64) {
    return Err("umulh requires 64-bit (X) registers".to_string());
}
```

## Test evidence

```
umulh_fixed_fields .............................. ok
umulh_register_fields_match_reference ........... ok
umulh_missing_operand_errors .................... ok
smulh_vs_umulh_only_sign_bit_differs ............ ok
umulh_rejects_non_register_operand_in_any_position ... ok
umulh_w_form_emits_identical_word_to_x_form ..... ok   (pins this bug)
umulh_silently_accepts_trailing_extra_operand ... ok
umulh_rejects_32bit_w_registers ................. FAILED   <-- this bug
```

## Related observation (low severity)

A second, additive property — `umulh_silently_accepts_trailing_extra_operand`
— shows `encode_umulh` performs **no upper-bound arity check**: it reads only
`operands[0..3]` via `get_reg` and ignores any surplus trailing operands, so an
input like `umulh x0, x1, x2, x3` is accepted and encoded as `umulh x0, x1, x2`
(instead of erroring on the unexpected 4th operand). This is consistent with the
rest of the encoder module (which uniformly lacks max-arity validation) and is
recorded here as a characterization, not a regression target.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/123
