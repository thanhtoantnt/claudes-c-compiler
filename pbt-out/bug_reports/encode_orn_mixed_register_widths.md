# Bug — `encode_orn` silently accepts mixed X/W register widths

**Function:** `encode_orn` — `src/backend/arm/assembler/encoder/data_processing.rs` (fn at line 910)
**PBT property:** `orn_rejects_mixed_register_widths` — **FAILS**

## Reproduction

```
cargo test --lib 'data_processing::tests::orn_rejects_mixed_register_widths'
```

Minimal failing input (proptest-shrunk): `rd = 0, rn = 0, rm = 0, mix = 0`
i.e. `orn x0, w0, w0` is accepted and encoded as a 64-bit ORN whose Rm/Rn fields are taken
from the W operands. The property also covers `orn w0, x0, x0` and the one-mixed case
`orn x0, x0, w0`.

## Spec

All operands of a shifted-register logical op must share the same register width. GAS rejects
`orn x0, w1, w2` with `Error: operand size mismatch` (and likewise llvm-mc).

## Root cause (`data_processing.rs:929-931`)

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;
let (rm, _) = get_reg(operands, 2)?;
let sf = sf_bit(is_64);
```

`is_64` (hence `sf`, bit 31) is derived **only** from operand 0. The widths of operands 1 and 2
are discarded (the `_` bindings). Nothing checks the three widths agree, so an X destination
paired with W sources (or vice-versa) is silently encoded into a single-width instruction.

## Suggested fix

Capture and compare the widths of all three register operands:
```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if rd64 != rn64 || rd64 != rm64 {
    return Err("orn operands must all be the same register width".to_string());
}
let sf = sf_bit(rd64);
```

## Impact

A typo'd `orn x0, w1, w2` assembles without error and produces a 64-bit instruction that reads
the intended (small) register numbers but with the wrong `sf` — a silent miscompilation. The
same latent bug class exists in the sibling encoders (`encode_eon`, `encode_bics`,
`encode_mvn`, `encode_logical` register path) which derive `sf` from operand 0 only.
