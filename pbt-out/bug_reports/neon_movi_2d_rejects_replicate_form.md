# Bug: `encode_neon_movi` `.2d` rejects the standard replicate immediate form

## Summary
The ARMv8-A ARM `MOVI Vd.2D, #imm` (cmode=1110, op=0) replicates `imm8` to every byte. The implementation's `.2d` branch instead requires every byte to be `0x00` or `0xFF` (a byte-mask semantic) and rejects valid standard replicate immediates like `#0x0101010101010101` (imm8=0x01).

## Witness
```
cargo test -- --ignored movi_2d_rejects_replicate_form
```
Fails: `movi v0.2d, #0x0101010101010101 is the standard replicate form (imm8=0x01); expected Ok`.

## Root cause
The `.2d` branch implements non-standard byte-mask validation instead of the ARM ARM's replicate semantic.

## Severity
MEDIUM — valid instructions are rejected; code generation cannot use the standard MOVI .2d form.
