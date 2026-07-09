# Bug: `encode_mov` dispatch silently drops `lsl #N` on `mov <Rd>, #imm`

**Law:** `MOV (wide immediate)` is an alias of `MOVZ`. The ARMv8-A ARM spells it
`MOV <Wd|WSP>, #<imm>{, LSL #<shift>}`, so `mov <Rd>, #imm, lsl #N` MUST encode
identically to `movz <Rd>, #imm, lsl #N` (i.e. `hw = N/16`, the 16-bit chunk
placed at bits `[16*hw + 15 : 16*hw]`). The two encoders must agree on the same
alias input.

**Impact:** Any assembly written as `mov x0, #0x1234, lsl #16` (intending to
materialise `0x12340000`) is silently mis-compiled to `movz x0, #0x1234`
(materialising `0x00001234`). The shift is dropped with no diagnostic, so the
produced machine code computes a **different 64-bit value** than the source
requested — a silent miscompilation. The defect is also reachable for invalid
inputs: `mov w0, #1, lsl #32` (UNALLOCATED for `sf=0`) is silently accepted
and emitted as `movz w0, #1` instead of being rejected.

**Function:** `encode_mov` — the `mov <Rd>, #imm` dispatch branch in
`src/backend/arm/assembler/encoder/data_processing.rs`.

**Detected by:** Differential oracle (sibling-implementation disagreement:
`encode_mov` vs `encode_movz`, which honours the shift), plus a reference KAT
and a negative contract.

**Minimal input:** `mov x0, #0x1234, lsl #16`
(operands: `[Reg("x0"), Imm(0x1234), Shift { kind: "lsl", amount: 16 }]`)

**Expected:** `Ok(Word(0xD2A24680))` — `movz x0, #0x1234, lsl #16`
(sf=1, opc=10, fixed=100101, **hw=1**, imm16=0x1234, rd=0). Verified with
`llvm-mc`:

```text
$ echo 'mov x0, #0x1234, lsl #16' | llvm-mc --triple=aarch64 -show-encoding
    movz   x0, #4660, lsl #16       // encoding: [0x80,0x46,0xa2,0xd2]
```

**Actual:** `Ok(Word(0xD2824680))` — `movz x0, #0x1234` (no shift)
(sf=1, opc=10, fixed=100101, **hw=0**, imm16=0x1234, rd=0). The `lsl #16` is
silently dropped, so the instruction materialises `0x00001234` instead of
`0x12340000`.

**Severity:** High — silent wrong-code on a reachable assembler input; no error
is reported.

## Root cause

The `mov <Rd>, #imm` branch of `encode_mov` reads only `operands[0]` (the
destination) and `operands[1]` (the immediate). It never inspects
`operands[2]`, so a trailing `Shift { kind: "lsl", amount }` operand — which the
parser does emit for `lsl #N` and which `encode_movz`/`encode_movk`/`encode_movn`
all consume — is invisible to the dispatcher:

```rust
// mov Xd, #imm -> movz or movn
if let Some(Operand::Imm(imm)) = operands.get(1) {
    let (rd, is_64) = get_reg(operands, 0)?;
    let imm = *imm;

    // Check if it can be a simple MOVZ
    if (0..=0xFFFF).contains(&imm) {
        let sf = sf_bit(is_64);
        let word = (sf << 31) | (0b10100101 << 23) | ((imm as u32 & 0xFFFF) << 5) | rd;
        return Ok(EncodeResult::Word(word));   // <-- operands[2] (Shift) ignored
    }
    ...
}
```

`encode_movz`, by contrast, computes `hw = amount / 16` from `operands.get(2)`,
so the two encoders disagree on the same logical (`mov` vs `movz` alias) input.

## Suggested fix

Either honour the shift (delegate the simple-MOVZ path to `encode_movz`, which
already validates/normalises `hw`), or reject a trailing shift with `Err`.
Minimal honour-the-shift version:

```rust
if (0..=0xFFFF).contains(&imm) {
    let sf = sf_bit(is_64);
    let hw = match operands.get(2) {
        Some(Operand::Shift { kind, amount }) if kind == "lsl" => *amount / 16,
        Some(Operand::Shift { .. }) => return Err("mov: only lsl shift is valid".into()),
        _ => 0,
    };
    if !is_64 && hw > 1 {
        return Err("mov: lsl shift for W-register must be #0 or #16".into());
    }
    let word = (sf << 31) | (0b10100101 << 23) | (hw << 21) | ((imm as u32 & 0xFFFF) << 5) | rd;
    return Ok(EncodeResult::Word(word));
}
```

## Regression test

Witnesses in
`src/backend/arm/assembler/encoder/data_processing_mov_dispatch_pbt.rs`
(all `#[ignore]`d so the default `cargo test` stays green; run with `--ignored`):

- `mov_dispatch_honours_lsl_like_movz` — differential: `encode_mov` ==
  `encode_movz` for `mov <Rd>, #imm, lsl #N`. Shrinks to `mov x0, #0, lsl #16`
  (`0xD2824680` vs `0xD2A24680`).
- `mov_dispatch_kat_lsl16` — reference KAT: `mov x0, #0x1234, lsl #16` ==
  `0xD2A24680`.
- `mov_dispatch_rejects_unallocated_w_lsl` — negative contract: `mov w0, #imm,
  lsl #>=32` must be `Err`. Shrinks to `mov w0, #0, lsl #32`.

Reproduce:

```bash
cargo test --lib data_processing_mov_dispatch_pbt -- --ignored
```

## Note on scope

This is a **distinct** defect from the three known `encode_mov{z,k,n}` classes
already reported (`encode_mov{z,k,n}_immediate_truncation.md`,
`encode_mov{z,k,n}_shift_normalization.md`, `encode_mov{z,k,n}_w_register_invalid_shift.md`):
those concern `encode_movz/movk/movn` *directly*; this one concerns the
`encode_mov` *dispatcher* dropping the shift operand before it ever reaches
`encode_movz`. The negative-immediate masking observed through dispatch
(`movz x0, #-1` → `imm16=0xffff`) is already covered by
`encode_movz_immediate_truncation.md` (whose regression property includes
`imm(-1)`) and is intentionally not re-reported here.
