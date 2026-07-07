# Bug: `encode_mul` silently accepts SP in any operand (encodes as multiply-by-zero)

**Severity:** High (silent miscompilation — wrong instruction emitted with no error)
**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Found by:** Property-based testing of `encode_mul` (characterization test + spec analysis)
**Category:** Silent acceptance of architecturally-undefined operand / no range validation

## Summary

`encode_mul` uses the shared `get_reg` helper to resolve register operands. `get_reg`
(in `encoder/mod.rs`) maps the names `"sp"` and `"wsp"` to register number **31**:

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    match name.as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr" => Some(31),
        ...
```

In the ARMv8 **MADD/MUL** encoding (`sf 0 0 11011 000 Rm 0 Ra Rn Rd`), operand field
value `31` is decoded as **XZR (zero register)**, *not* SP. MADD/MUL does **not** have
an SP-using variant (unlike ADD/SUB which have a separate "add/sub, immediate" and
extended-register form that honours SP). Per the ARMv8 ARM, supplying SP in any of
Rd/Rn/Rm/Ra of a data-processing register operation is **UNPREDICTABLE / unallocated**.

Consequently `mul x0, x1, sp` is **silently** encoded as `mul x0, x1, xzr` — i.e. a
multiply-by-zero — with `Ok(Word(...))` and no diagnostic.

## Evidence

Characterization test `mul_sp_in_rm_is_silently_accepted_as_xzr` (added to the
`data_processing.rs` test module) prints, for `mul x0, x1, sp`:

```
mul x0,x1,sp -> Ok(Word(2602531872))
  rm field = 31 (== 31 means XZR, not SP)
```

`2602531872 == 0x9B217C20`; decoding per ARMv8 MADD gives `sf=1, Rm=31(XZR), o0=0,
Ra=31(XZR), Rn=1, Rd=0` → `madd x0, x1, xzr, xzr` == `mul x0, x1, xzr`.

The same silent aliasing affects SP in **Rd** and **Rn** as well, since all three are
resolved through the same `get_reg` → `parse_reg_num` path. e.g. `mul x0, sp, x1` would
encode `Rn=31`→XZR, silently computing `x0 = x1 * 0`.

## Expected vs actual

| Input | Expected (ARMv8) | Actual |
|-------|------------------|--------|
| `mul x0, x1, sp` | `Err` (SP not permitted in MUL/MADD operands) | `Ok(...)` → `mul x0, x1, xzr` (mul-by-0) |
| `mul sp, x1, x2` | `Err` (SP not permitted as MUL destination)   | `Ok(...)` → `mul xzr, x1, x2` (result discarded) |
| `mul x0, sp, x2` | `Err` (SP not permitted in MUL source)        | `Ok(...)` → `mul x0, xzr, x2` (mul-by-0) |

## Why this matters

- **Silent miscompilation.** A program containing `mul x0, x1, sp` (whether hand-written
  or compiler-emitted) gets assembled to an instruction that always yields 0 instead of
  the product, with no assembler error to catch the mistake.
- **No spec basis for the aliasing.** Unlike ADD/SUB/ADRP/LDR (immediate/extended forms),
  MADD/MSUB/UDIV/SDIV/shift-by-register etc. have *no* SP-using form; register field 31
  is unambiguously XZR. There is no encoding it could "really" mean.
- **Shared-helper scope.** The root cause is `parse_reg_num`/`get_reg` unconditionally
  mapping `sp`/`wsp`→31, which is correct for SP-aware instructions but wrong for every
  data-processing-3-source / 2-source / logical-register / shift instruction. The bug
  therefore also affects `encode_madd`, `encode_msub`, `encode_mneg`, `encode_div`,
`encode_mulh`, `encode_smulh`, `encode_logical` (register form), `encode_shift`
  (register form), etc. — though this report is scoped to `encode_mul`.

## Suggested fix

Reject SP/WSP (and, separately, the wrong register width, e.g. `mul x0, w1, x2`) in
`encode_mul` before encoding — e.g. add an operand-class check, or introduce a
`get_reg_no_sp` helper for instructions whose encoding reserves field 31 for XZR:

```rust
pub(crate) fn encode_mul(operands: &[Operand]) -> Result<EncodeResult, String> {
    if let Some(Operand::RegArrangement { .. }) = operands.first() {
        return encode_neon_mul(operands);
    }
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    reject_sp(&operands[0..3])?;          // <-- new
    reject_width_mix(operands, is_64)?;   // <-- new (optional, related)
    ...
}
```

## Repro

```bash
cargo test --lib \
  backend::arm::assembler::encoder::data_processing::tests::mul_sp_in_rm_is_silently_accepted_as_xzr \
  -- --nocapture
```
Output shows `Ok(Word(...))` with Rm field == 31 instead of an `Err`.

## Status

Open. No evidence found that this behavior is intentional; the encoder's own design
(distinguishing SP-aware ADD/SUB/extended forms from XZR-only MUL) implies SP should be
rejected here.
