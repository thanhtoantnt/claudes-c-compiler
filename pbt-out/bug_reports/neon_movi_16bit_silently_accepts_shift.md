# Bug: `encode_neon_movi` `.4h`/`.8h` forms silently accept and drop shift operands

## Summary
MOVI's 16-bit element forms (`.4h`/`.8h`) have only the unshifted encoding (`cmode=1000`). The encoder's `.4h`/`.8h` branch ignores any trailing `Operand::Shift` and returns `Ok` with `cmode=1000`, silently dropping the shift. A conforming assembler rejects `movi v0.4h, #5, lsl #8`.

## Witness
```
cargo test -- --ignored movi_16bit_silently_accepts_shift_operand
```
Fails: `movi v0.4h, #5, lsl #8 must be rejected but got Ok(...)`.

## Root cause
The `.4h`/`.8h` branch does not check `operands.len() > 2` or validate the absence of a shift operand.

## Severity
MEDIUM — silent misencoding; the instruction executes as the unshifted form, silently changing program semantics.
