# BUG: `encode_ubfiz` panics / emits UNDEFINED encodings for invalid immediates (no range validation)

## Target
`src/backend/arm/assembler/encoder/bitfield.rs` → `encode_ubfiz`, lines ~76–88:

```rust
pub(crate) fn encode_ubfiz(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;
    let regsize = if is_64 { 64u32 } else { 32 };
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);
    let imms = width - 1;                                       // <-- panics when width == 0
    let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22)
             | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Finding
`UBFIZ Rd, Rn, #lsb, #width` is the alias for `UBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)`,
placing `immr` into bits `[21:16]` and `imms` into bits `[15:10]` (both 6-bit fields).
The ARM ARM (Bitfield, §C4.1.66 UBFIZ) constrains the operands to `0 <= lsb <= regsize-1`
and `1 <= width <= regsize - lsb` (regsize = 64 for X, 32 for W). The encoder performs
**no** range validation. As a result invalid immediates are not rejected:

- `width == 0` underflows `imms = width - 1` (`u32`), which is a **debug-build panic**
  (`attempt to subtract with overflow`) that aborts the entire encoder instead of
  returning `Err`.
- Out-of-range `lsb`/`width` and negative immediates are silently accepted: the
  `as u32` cast wraps negatives to huge values and the OR into `word` lets
  `imms`/`immr` overflow past their 6-bit fields into the `Rn`/opcode bits,
  emitting a corrupt/UNDEFINED instruction word as `Ok`.

This is one root cause (no validation) with a spectrum of symptoms; the panic is
the most severe. The same missing-validation defect exists in the sibling
bitfield encoders (`encode_ubfx`, `encode_ubfm`, `encode_sbfm`, `encode_sbfx`,
`encode_sbfiz`, `encode_bfm`, `encode_bfi`, `encode_bfxil`).

## Reproduction
Property `prop_rejects_out_of_range_immediates` in module `prop_encode_ubfiz_tests`
fails on its first iteration:

```
minimal failing input: is_64 = false, over_lsb = 64, over_width = 65, neg = -3
Test failed: width=0 (imms underflow) should be Err but PANICKED at bitfield.rs:85
```

Hand-replicated encodings (64-bit, `UBFIZ x0, x1, #lsb, #width`) confirming the
silent corruption and the panic:

| input            | produced      | expected |
|------------------|---------------|----------|
| `#1, #1` (valid) | `0xd37f0000`  | Ok ✓     |
| `#0, #0`         | **panic**     | Err      |
| `#100, #1`       | `0xd35c0000`  | Err      |
| `#1, #200`       | `0xd37f1c00`  | Err      |
| `#-1, #1`        | `0xd3410000`  | Err      |
| `#1, #-5`        | `0xffffe800`  | Err      |

## Impact
- `UBFIZ x0, x1, #0, #0` (and any `width == 0`) **crashes** the assembler — a
  denial-of-service panic in the compilation pipeline.
- Otherwise-invalid `UBFIZ` is assembled into a malformed word with no error
  signal, so downstream consumers (objdump, JIT/emulator, real CPU) disagree on
  its meaning (UNDEFINED behaviour).

## Suggested fix
Validate `lsb`/`width` against `regsize` before encoding:

```rust
let lsb_i = get_imm(operands, 2)?;
let width_i = get_imm(operands, 3)?;
if lsb_i < 0 || width_i < 1 || (lsb_i as u32) >= regsize
    || (lsb_i as u32) + (width_i as u32) > regsize {
    return Err(format!(
        "ubfiz: lsb/width out of range for {}-bit register: lsb={}, width={}",
        regsize, lsb_i, width_i
    ));
}
let lsb = lsb_i as u32;
let width = width_i as u32;
```

Apply the same guard to the sibling `encode_*` bitfield encoders.

## Properties (`prop_encode_ubfiz_tests`)
- `prop_ubfiz_field_placement` — PASS
- `prop_immr_is_neg_lsb_mod_regsize` — PASS
- `prop_ubfiz_equals_ubfm_with_converted_immediates` — PASS
- `prop_width_changes_only_sf_n_immr` — PASS
- `prop_rejects_out_of_range_immediates` — **FAIL** (the finding)
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/156
