# Bug: `encode_bfxil` silently accepts out-of-range `lsb`/`width` (no range validation)

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_bfxil`
**Found by:** property `prop_encode_bfxil_tests::prop_rejects_out_of_range_operands`
**Severity:** high (silent mis-encoding — wrong machine code emitted as `Ok`)

## Minimal inputs

```
BFXIL X0, X1, #100, #1     # lsb > 63 (overflow immr into N bit)
BFXIL X0, X1, #63, #2      # lsb+width-1 = 64 > 63 (overflow imms into Rn field)
BFXIL X0, X1, #-1, #1      # negative immediate wraps via `as u32`
```

## Expected

The assembler rejects each operand with a clean `Err`. ARM ARM (BFXIL /
Bitfield) constrains:

- 64-bit: `0 <= lsb <= 63`, `1 <= width <= 64 - lsb`
- 32-bit: `0 <= lsb <= 31`, `1 <= width <= 32 - lsb`

so that `immr == lsb` and `imms == lsb + width - 1` each fit their 6-bit fields
`[21:16]` / `[15:10]`. Any operand outside these ranges must be diagnosed, not
encoded.

## Actual

The encoder casts with `as u32` and ORs the values straight into the word with
**no validation**:

```rust
let immr = lsb;
let imms = lsb + width - 1;
let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23)
         | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

Consequences:

- `lsb >= 64` overflows `immr` into the **N bit [22]** (and higher opcode bits).
- `lsb + width - 1 >= 64` overflows `imms` into the **Rn field [9:5]**.
- Negative immediates wrap via the `as u32` cast into the upper opcode bits.

The result is a wrong instruction returned as `Ok(EncodeResult::Word(..))` —
no diagnostic, no signal to the caller.

## Impact

Silently emits incorrect AArch64 machine code for malformed `BFXIL` operands.
Because the broken word still parses as a valid instruction shape downstream,
the error is invisible until runtime / disassembly.

## Fix

Add a guard before building the word:

```rust
let regsize = if is_64 { 64u32 } else { 32 };
if lsb >= regsize || width == 0 || lsb + width > regsize {
    return Err(format!("BFXIL: lsb/width out of range (lsb={}, width={}, regsize={})",
                       lsb, width, regsize));
}
```

## Scope

The identical `immr = lsb; imms = lsb + width - 1;` pattern with no validation
is shared by the sibling extract aliases `encode_ubfx` and `encode_sbfx`.
The raw BFM-family encoders (`encode_bfm`, `encode_ubfm`, `encode_sbfm`) have
the same missing-validation problem for their raw 6-bit `immr`/`imms`. A shared
helper is advisable.

## Test evidence

`prop_rejects_out_of_range_operands` asserts that each of

```
(reg_width, 1, "lsb==regsize"),
(over_lsb,  1, "lsb>regsize (immr overflow into N bit)"),
(reg_width-1, 2, "lsb+width>regsize (imms overflow)"),
(0, over_width, "width>regsize"),
(neg, 1, "negative lsb"),
(0, neg, "negative width"),
```

returns `Err`. They currently return `Ok(..)` (the `#0,#0` panic case is
filed separately in `encode_bfxil_panic_on_zero_width.md`).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/137
