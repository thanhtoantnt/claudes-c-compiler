# Bug: `encode_neon_movi` `.2d` form emits MVNI opcode bit (bit 29)

## Summary
`movi v0.2d, #0` emits `0x6F00E400` (bit 29 = 1, the MVNI `op` bit) instead of the correct `0x4F00E400`. Combined with `cmode=1110`, `op=1` is an **UNALLOCATED** encoding in the ARMv8-A ARM — the emitted word does not decode to any defined instruction.

## Witness
```
cargo test -- --ignored movi_2d_emits_mvni_op_bit
```
Fails: `got 0x6F00E400 with bit 29 set`.

## Root cause
The `.2d` branch in `encode_neon_movi` sets the top byte to `0x6F` (which is the MVNI prefix) instead of `0x4F` (MOVI prefix). Every other arrangement correctly uses `0x0F`/`0x4F`.

## Severity
HIGH — emits UNALLOCATED instruction word; undefined behavior on hardware.

## Suggested fix
Change the `.2d` branch prefix from `0x6F` to `0x4F` (clear bit 29).
