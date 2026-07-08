# PBT Coverage — `encode_mneg` (data_processing.rs)

## Target
`encode_mneg(operands: &[Operand]) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/data_processing.rs`.

MNEG `<Rd>,<Rn>,<Rm>` is the ARMv8 alias of `MSUB <Rd>,<Rn>,<Rm>,<XZR>`
(MSUB with Ra = XZR). Encoded word:
`sf 0 0 11011 o1=0 00 Rm o0=1 Ra=11111 Rn Rd`.

## Properties (module `mneg_props`, 6 tests, all PASS)
| # | Property | Oracle |
|---|----------|--------|
| P1 | `mneg_reference_encoding` — full word == `0x9B00FC00`/`0x1B00FC00` OR'd with Rm/Rn/Rd | reference constant |
| P2 | `mneg_field_placement` — sf, fixed[30:21]=`0b0011011000`, o0(bit15)=1, Ra=`11111`, Rm/Rn/Rd | field extraction |
| P3 | `mneg_sf_tracks_destination_width_only` — sf from operand 0 only, sources ignored | structural |
| P4 | `mneg_differs_from_mul_only_in_o0` — `MNEG ^ MUL == 1<<15` | differential (MADD vs MSUB) |
| P5 | `mneg_equals_msub_with_ra_xzr` — byte-identical to `MSUB Xd,Xn,Xm,XZR` | differential (alias def) |
| P6 | `mneg_rejects_missing_or_non_register_operands` — <3 regs / non-reg → Err | negative contract |

## Result
All 6 properties pass. No functional finding — the encoding matches the
ARMv8 ARM MSUB/MNEG alias bit-layout exactly, confirmed by two independent
differential oracles (P4 vs `encode_mul`, P5 vs `encode_msub`).

## Note (not a bug, consistent codebase convention)
Like `encode_mul` / `encode_madd`, `is_64` is read **only** from the
destination register (operand 0); source widths are not validated, so
`mneg x0, w1, w2` encodes as 64-bit without error. This is the established
design of every multiply-family encoder here (and is codified by the
existing `mul` tests' own "sf tracks destination width only" property),
so it is treated as intended rather than a defect.
