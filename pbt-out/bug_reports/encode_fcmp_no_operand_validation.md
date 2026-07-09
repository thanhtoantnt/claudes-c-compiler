# Bug Report: `encode_fcmp` does not validate the second operand's precision/bank

**Location:** `src/backend/arm/assembler/encoder/fp_scalar.rs`, function `encode_fcmp` (line 160)

## Summary

`encode_fcmp` silently accepts illegal operands and emits an encoding that does
**not** match the source text. It derives the `ftype` (precision) field from
`operands[0]` only and never inspects `operands[1]`'s register prefix, so a
mixed-precision operand like `FCMP D0, S0` is rewritten into `FCMP D0, D0`.

Per the ARMv8 ARM, FCMP requires homogeneous-precision FP-register operands:

```
FCMP  <Sn>, <Sm>          // single-precision
FCMP  <Dn>, <Dm>          // double-precision
```

## Root cause

```rust
pub(crate) fn encode_fcmp(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rn_name.starts_with('d');                 // only operand[0]
    let ftype = if is_double { 0b01 } else { 0b00 };
    ...
    let (rm, _) = get_reg(operands, 1)?;                      // operands[1] prefix never checked
    let word = (0b00011110 << 24) | (ftype << 22) | (1 << 21)
             | (rm << 16) | (0b001000 << 10) | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)`; the `is_64` flag is discarded for both
operands, and `operands[1]`'s name is never re-examined to confirm it matches
the precision of `operands[0]`.

## Minimal failing input

PBT witness — failing, shrunk property
`prop_fcmp_rejects_mismatched_precision_and_bank`
(`src/backend/arm/assembler/encoder/fp_scalar.rs:1102`):

- **Seed:** `cc 297bb8fdde1ac10154d3ab5cffb849c73f76de8cdd99db65018c30ae54bdb799` (shrinks to `n = 0`)
- **Counterexample (shrunk):** `encode_fcmp(&[Operand::Reg("d0"), Operand::Reg("s0")])`
- **Actual:** `Ok(Word(509616128))` → `0x1E602000` → decodes to **`FCMP D0, D0`** (ftype=01, Rn=0, Rm=0)
- **Expected:** `Err(...)` (mixed precision `Dn, Sm` is not encodable)
- **Reproduce:** `cargo test --lib prop_fcmp_rejects_mismatched_precision_and_bank`
  (the seed is persisted in `proptest-regressions/backend/arm/assembler/encoder/fp_scalar.txt` and replayed automatically)

## Impact

- The `S0` operand is dropped: the emitted instruction compares a register
  against **itself** (`FCMP D0, D0`) rather than against the named operand.
  This is a silent miscompile — the assembler returns `Ok` with a valid-looking
  word that carries the wrong semantics.
- The reverse case `FCMP S0, D0` encodes to `0x1E202000` = `FCMP S0, S0`
  (same drop/coerce behavior, confirmed by decoding).
- GP-bank operands (`FCMP X0, X0`) are likewise accepted and coerced, though
  that path is exercised by the same property rather than separately witnessed.

## Suggested fix

After resolving both registers, validate that both operands are FP-bank and
share the same precision:

```rust
let rm_name = match &operands[1] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("fcmp: expected register operand".into()),
};
let rm_is_double = rm_name.starts_with('d');
let rm_is_single = rm_name.starts_with('s');
if !rm_is_double && !rm_is_single {
    return Err("fcmp requires FP-register operands".into());
}
if is_double != rm_is_double {
    return Err("fcmp operands must have matching precision".into());
}
```
