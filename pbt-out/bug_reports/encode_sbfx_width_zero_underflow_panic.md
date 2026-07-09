# Bug Report: `encode_sbfx` panics (debug) / silently corrupts (release) when `width == 0`

**Location:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_sbfx`, line 30

## Summary

The alias computation

```rust
let imms = lsb + width - 1;
```

uses unchecked `u32` arithmetic. When the user-supplied `width` immediate is `0`,
this underflows `0u32 - 1`:

- **Debug builds:** Rust traps with `attempt to subtract with overflow`, so the
  **assembler process aborts** on the input `sbfx x0, x1, #0, #0` instead of
  returning `Err`.
- **Release builds:** the value silently wraps to `0xFFFF_FFFF` and is OR-ed into
  the `imms [15:10]` field, overflowing into `Rn [9:5]` and `Rd [4:0]` and
  emitting a garbage word with no diagnostic.

## Root cause

```rust
pub(crate) fn encode_sbfx(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;
    let width = get_imm(operands, 3)? as u32;      // no range check, no min check
    ...
    let imms = lsb + width - 1;                     // <-- panics on width==0 (debug)
    let word = ... | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_imm` returns the raw `i64`; the `as u32` cast truncates with no validation,
and `width == 0` reaches the subtraction unchecked.

## Minimal reproducer

```rust
encode_sbfx(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(0),   // lsb
    Operand::Imm(0),   // width  -> lsb + width - 1 = 0u32 - 1  => underflow
]);
```

```
thread '...' panicked at src/backend/arm/assembler/encoder/bitfield.rs:30:16:
attempt to subtract with overflow
minimal failing input: is_64 = false, big_lsb = 64, big_width = 65, neg = -3
```

(The minimal input above comes from property `prop_rejects_out_of_range_lsb_width`,
whose very first assertion — `mk(0, 0)` — is the panic trigger.)

## Expected vs actual

- **Expected:** `Err` (a bitfield extract with `width == 0` is meaningless and
  the ARMv8 ARM requires `width >= 1`, ARM ARM §C4.1.69).
- **Actual:** debug = panic (process abort); release = `Ok(Word(..))` with a
  corrupted `imms`/`Rn`/`Rd`.

## Impact

- A malformed `.s` containing `sbfx x0, x1, #0, #0` (or any alias feeding
  `width == 0`) **crashes the assembler** under the default debug test/dev build.
- Under release the assembler silently produces a wrong, architecturally-invalid
  instruction with no error reported to the user.
- The same `lsb + width - 1` unchecked pattern is shared by `encode_ubfx`,
  `encode_bfxil`, and `encode_bfi`, so the panic is reachable through those
  aliases too.

## Suggested fix

Reject `width == 0` (and the wider range — see companion report
`encode_sbfx_no_lsb_width_range_validation.md`) before the arithmetic:

```rust
if width == 0 {
    return Err("SBFX: width must be >= 1".into());
}
// or use checked arithmetic:
let imms = lsb.checked_add(width).and_then(|s| s.checked_sub(1))
    .ok_or("SBFX: lsb+width-1 overflow")?;
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/161
