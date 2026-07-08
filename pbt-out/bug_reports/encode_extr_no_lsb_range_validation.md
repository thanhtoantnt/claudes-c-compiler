# Bug Report: `encode_extr` silently accepts out-of-range `#lsb` immediate

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_extr`
**Severity:** Medium (silent miscompilation — produces a valid-looking but semantically wrong instruction word, no error surfaced to the assembler caller)

## Summary

`encode_extr` reads its immediate operand with `get_imm(...)? as u32` and directly
ORs it into the 32-bit encoding word with **no range validation**:

```rust
pub(crate) fn encode_extr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let lsb = get_imm(operands, 3)? as u32;          // <-- no range check
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    // EXTR: sf 0 0 100111 N 0 Rm imms Rn Rd
    let word = (sf << 31) | (0b00100111 << 23) | (n << 22) | (rm << 16)
        | (lsb << 10) | (rn << 5) | rd;              // <-- lsb<<10 can overflow
    Ok(EncodeResult::Word(word))
}
```

Per the ARM Architecture Reference Manual (`EXTR`, "Extract register"), `lsb` is
encoded in the 6-bit `imms` field (`[15:10]`) with these constraints:

* 64-bit form (`sf=1, N=1`): `0 <= lsb <= 63`
* 32-bit form (`sf=0, N=0`): `0 <= lsb <= 31`   (i.e. `imms[5]` must be 0)

An assembler **must reject** any value outside these ranges. The current code
performs no check, so:

* `lsb = 64` (or larger) — `(64u32 << 10) = 0x0001_0000` is **bit 16**, which is
  the low bit of the `Rm` field (`[20:16]`). The overflow silently corrupts the
  register operand rather than being rejected. For very large `lsb`, the garbage
  propagates further into `N` / opcode / `sf`.
* `32 <= lsb <= 63` in the 32-bit (W) form — sets `imms[5]`. The ARM ARM marks
  this encoding UNDEFINED for the 32-bit variant; a conforming assembler must
  reject it.
* Negative `lsb` — the `i64 as u32` cast wraps (e.g. `-1 -> 0xFFFF_FFFF`), so
  `(lsb << 10)` splatters garbage across `Rn`, `Rm`, `N`, and the opcode bits.

## Root Cause

The immediate is taken straight from `get_imm` (an `i64`), cast to `u32` with
`as`, and shifted into the 6-bit `imms` field without any bounds check. There is
no width-dependent (`0..=63` vs `0..=31`) validation, and no rejection of
negative inputs before the wrapping cast.

## Reproduction

Property `prop_encode_extr_tests::prop_rejects_out_of_range_lsb` fails on the
**first** generated input:

```
minimal failing input: is_64 = true, big_lsb = 64, mid_lsb = 32, neg_imm = -1
panicked: lsb=64 (>63) should be rejected, got Ok(Word(327352352))
```

`Word(327352352)` is `0x1383_0020`. Decoding:

```
0x13830020 = 0001 0011 1000 0011 0000 0000 0010 0000
 sf[31]=0  opc[30:29]=00  100111[28:23]  N[22]=0  o0[21]=0
 Rm[20:16]=00011 (=3)  imms[15:10]=000000 (=0)  Rn[9:5]=00001 (=1)  Rd[4:0]=00000 (=0)
```

The intended `lsb=64` did **not** land in the `imms` field (which reads back as
0); it instead set bit 16, the low bit of the `Rm` field, turning `x2` (Rm=2)
into `x3` (Rm=3). The encoder returned `Ok` with a word whose fields no longer
correspond to the inputs — a silent miscompile. GNU `as` rejects
`extr x0, x1, x2, #64` with `immediate out of range`.

Concrete commands:

```
cargo test --lib prop_encode_extr_tests::prop_rejects_out_of_range_lsb
```

## Impact

Silent miscompilation: out-of-range `EXTR` operands produce a 32-bit word that
*looks* valid (well-formed `EXTR` opcode) but whose `Rm`/`Rn`/`imms` fields no
longer match the source text. The wrong source register is encoded and the
extract width is wrong, so the emitted instruction performs a completely
different operation from what the assembly says. No error is surfaced to the
assembler caller. The 32-bit `imms[5]` case additionally emits an
architecturally UNDEFINED instruction.

## Suggested Fix

Validate `lsb` against the register width before encoding, e.g.:

```rust
let lsb_i64 = get_imm(operands, 3)?;
if lsb_i64 < 0 {
    return Err(format!("extr: lsb #{} must be non-negative", lsb_i64));
}
let lsb = lsb_i64 as u32;
let max = if is_64 { 63u32 } else { 31u32 };
if lsb > max {
    return Err(format!(
        "extr: lsb #{} out of range (0..={}) for {}-bit register",
        lsb, max, if is_64 { 64 } else { 32 }
    ));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_lsb` (module `prop_encode_extr_tests`,
`src/backend/arm/assembler/encoder/bitfield.rs:2620`).

Minimal failing test case:

```rust
#[test]
fn regression_extr_lsb_64_not_rejected() {
    let ops = vec![
        Operand::Reg("x0".into()),
        Operand::Reg("x1".into()),
        Operand::Reg("x2".into()),
        Operand::Imm(64),            // lsb out of the 6-bit [0,63] range
    ];
    assert!(encode_extr(&ops).is_err(),
        "lsb=64 must be rejected, got {:?}", encode_extr(&ops));
}
```

## Scope

This is the same `as u32`-no-validation bug class already filed against the
sibling bitfield encoders (`encode_ubfx`, `encode_ubfm`, `encode_sbfm`,
`encode_bfm`, `encode_sbfiz`, `encode_ubfiz`, `encode_bfi`, `encode_bfxil`,
`encode_sbfx`). A shared `check_imm_range(value, bits, is_64)` helper would
address all of them.

## Test status (this campaign)

| Property | Result |
|---|---|
| `prop_extr_field_placement` (structural + N==sf + o0=0) | PASS |
| `prop_extr_canonical_encoding` (reference: `0x93C21420`) | PASS |
| `prop_width_changes_only_sf_and_n` (width differential) | PASS |
| `prop_deterministic` (purity) | PASS |
| `prop_rejects_malformed_operands` (negative contract) | PASS |
| `prop_rejects_out_of_range_lsb` (negative contract) | **FAIL — this bug** |
