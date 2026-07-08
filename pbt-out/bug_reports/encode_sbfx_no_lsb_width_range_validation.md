# Bug Report: `encode_sbfx` performs no range validation on `lsb` / `width`

**Location:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_sbfx`

## Summary

`encode_sbfx` accepts any `i64` for its `#lsb` and `#width` immediates — values
that are negative, equal to or greater than the register size, or that make
`lsb + width` exceed the register size — and emits a (corrupted) word with `Ok`,
instead of rejecting them with `Err`. Only `width == 0` additionally traps as an
overflow panic (tracked in the companion report
`encode_sbfx_width_zero_underflow_panic.md`); every other out-of-range case is
silently mis-encoded.

## Architecture contract (ARMv8 ARM §C4.1.69)

```
SBFX <Xd>, <Xn>, #<lsb>, #<width>   →   SBFM <Xd>, <Xn>, #<lsb>, #(lsb+width-1)
```

- 64-bit: `0 <= lsb <= 63`, `1 <= width <= 64 - lsb`
- 32-bit: `0 <= lsb <= 31`, `1 <= width <= 32 - lsb`

Both `immr` and `imms` are 6-bit fields (`[21:16]` and `[15:10]`), so values
outside these ranges overflow the field and corrupt neighbouring fields
(`imms` overflows into `Rn [9:5]` / `Rd [4:0]`; the `as u32` cast on a negative
`i64` produces a huge value that overflows the opcode bits).

## Root cause

```rust
pub(crate) fn encode_sbfx(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;     // no range check
    let width = get_imm(operands, 3)? as u32;   // no range check
    ...
    let immr = lsb;
    let imms = lsb + width - 1;
    let word = (sf << 31) | (0b100110 << 23) | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))                // never Err for bad lsb/width
}
```

There is no bounds check between `get_imm` and the field packing.

## Minimal examples

```rust
// 32-bit, lsb == regsize (out of range) -> should be Err, returns Ok(0x13XX_X...)
encode_sbfx(&[Reg("w0"), Reg("w1"), Imm(32), Imm(1)]);

// width > regsize -> should be Err, returns Ok with corrupted imms
encode_sbfx(&[Reg("x0"), Reg("x1"), Imm(0), Imm(65)]);

// negative lsb -> `as u32` wraps to ~0, overflowing the opcode bits
encode_sbfx(&[Reg("x0"), Reg("x1"), Imm(-3), Imm(1)]);
```

Each returns `Ok(EncodeResult::Word(..))` instead of `Err`.

## Expected vs actual

- **Expected:** `Err` for any `lsb < 0`, `lsb >= regsize`, `width <= 0`, or
  `lsb + width > regsize`.
- **Actual:** `Ok(Word(..))` with the overflowed `immr`/`imms` bits silently
  corrupting `Rn`/`Rd` (and, for negatives, the upper opcode bits).

## Impact

The assembler emits instructions that do not correspond to the source text and
are not valid `SBFM` encodings, with no diagnostic. This is the same class of
defect already filed for the raw forms (`encode_sbfm_no_imm_range_validation.md`,
`encode_bfm_no_range_validation.md`, `encode_ubfm_no_range_validation.md`).

## Suggested fix

Validate against the register size before packing:

```rust
let regsize = if is_64 { 64 } else { 32 };
if lsb >= regsize {
    return Err(format!("SBFX: lsb {} out of range [0, {})", lsb, regsize));
}
if width == 0 || lsb + width > regsize {
    return Err(format!(
        "SBFX: width {} invalid for lsb {} (regsize {})", width, lsb, regsize
    ));
}
```

## Verification

Property `prop_rejects_out_of_range_lsb_width` in module `prop_encode_sbfx_tests`
asserts all of the above cases must return `Err`. It fails (the `width == 0`
case panics first; the rest return `Ok`).
