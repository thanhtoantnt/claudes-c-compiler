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

---

# PBT Coverage — `encode_sxth` (data_processing.rs)

## Target
`encode_sxth(operands: &[Operand]) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/data_processing.rs`.

`SXTH <Rd>,<Rn>` is the ARMv8 alias of `SBFM <Rd>,<Rn>,#0,#15`.
Encoded word: `sf 00 100110 N immr=0 imms=15 Rn Rd`.

## Properties (module `sxth_props`, 6 tests; 5 PASS, 1 FAIL documenting a bug)
| # | Property | Oracle | Result |
|---|----------|--------|--------|
| P1 | `sxth_reference_encoding` — full word == `0x93403C00`(64)/`0x13003C00`(32) OR'd Rn/Rd | llvm-mc-18 differential | PASS |
| P2 | `sxth_field_placement` — sf, opc=00, fixed=`100110`, N==sf, immr=0, imms=15, Rn/Rd | field extraction | PASS |
| P3 | `sxth_source_width_irrelevant_for_64bit_destination` — `xN,xM` == `xN,wM` | llvm-mc canonicalization | PASS |
| P4 | `sxth_rejects_too_few_operands` — <2 regs → Err | negative contract | PASS |
| P5 | `sxth_rejects_non_register_operands` — Imm in pos 0/1 → Err | negative contract | PASS |
| P6 | `sxth_rejects_w_destination_with_x_source` — `sxth w0,x0` must be Err | negative contract (spec/llvm-mc) | **FAIL** |

## Result
Encoding of legal forms is correct (P1–P3, cross-checked against `llvm-mc-18`
for both widths and the full register range). One functional finding: see
`pbt-out/bug_reports/encode_sxth_silent_mixed_width_source.md` — the encoder
silently accepts `sxth wN, xN` (32-bit destination + 64-bit source), which the
reference assembler rejects. Sibling encoders `encode_sxtb`, `encode_uxth`,
`encode_uxtb` share the defect.
