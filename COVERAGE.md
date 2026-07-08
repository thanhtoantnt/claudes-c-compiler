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

---

# PBT Coverage — `encode_uxth` (data_processing.rs)

## Target
`encode_uxth(operands: &[Operand]) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/data_processing.rs`.

UXTH `<Wd>,<Wn>` is the ARMv8 alias of `UBFM <Wd>,<Wn>,#0,#15`. It is a
**32-bit-only** alias — there is no 64-bit UXTH form. Valid word:
`0 10 100110 0 immr=000000 imms=001111 Rn Rd` = `0x53003C00 | (Rn<<5) | Rd`.

## Properties (module `uxth_props`, 6 tests: 5 PASS, 1 FAILS — real bug)
| # | Property | Oracle |
|---|----------|--------|
| P1 | `uxth_reference_encoding_32bit` — word == `0x53003C00` OR'd with Rn/Rd | reference constant (llvm-mc) |
| P2 | `uxth_field_placement` — sf=0, opc=10, fixed=`100110`, N=0, immr=0, imms=15, Rn/Rd | field extraction |
| P3 | `uxth_source_width_irrelevant_for_32bit_destination` — `uxth w0,w0`==`uxth w0,x0` | structural |
| P4 | `uxth_rejects_too_few_operands` — <2 regs → Err | negative contract |
| P5 | `uxth_rejects_non_register_operands` — `Operand::Imm` in either slot → Err | negative contract |
| P6 | `uxth_rejects_64bit_destination_form` — `uxth xN,xM` → Err | negative contract (llvm-mc) |

## Result
The legal 32-bit form encodes correctly (P1–P3, cross-checked against
`llvm-mc-18` across the full W register range). **One functional finding,
PROVEN by failing P6:** `encode_uxth` silently accepts the architecturally
invalid `uxth xN, xM` (64-bit destination) and emits `0xD3403C00`, which
disassembles as `ubfx xN, xM, #0, #16` — a different instruction. See
`pbt-out/bug_reports/encode_uxth_silent_64bit_destination.md`.

Note: this is a **distinct** defect from the `sxth` mixed-width bug — UXTH has
no 64-bit form at all, whereas SXTH's 64-bit form is legal.

## `encode_rev16` — bitfield.rs (NO FINDING; clean pass)

**Target:** `pub(crate) fn encode_rev16(&[Operand]) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/bitfield.rs`.

**Module:** `prop_encode_rev16_tests` (appended to `bitfield.rs`). 5 properties,
all passing at `PROPTEST_CASES=2000`:

| Property | Oracle | Result |
|---|---|---|
| `prop_field_placement` | Structural — pins sf[31], bit30=1/bit29=0, bits[28:21]=0xD6, bits[20:16]=0, opc[15:10]=000001, Rn[9:5], Rd[4:0] | pass |
| `prop_matches_arm_reference` | Reference word — `0xDAC0_0400` (X) / `0x5AC0_0400` (W) `\| (rn<<5) \| rd` | pass |
| `prop_width_changes_only_sf` | Register-width differential — X vs W XOR == `MASK_SF` only | pass |
| `prop_xor_rev_confined_to_opc` | Differential vs `encode_rev` — XOR confined to opc[15:10], equals `(rev_opc ^ 0b000001)<<10` | pass |
| `prop_rejects_malformed_operands` | Negative contract — missing/non-register operands → Err | pass |

**Verdict — no defect.** `encode_rev16` is correct. Per the ARM ARM
(Data-processing (1 source)), REV16 uses the **same** `opc = 000001` for both
the 32-bit (W) and 64-bit (X) forms, so the register width changes **only**
bit `sf[31]`. The encoder does exactly that. This is the key contrast with
`encode_rev32` (which has a real bug: it hardcodes sf=1 / opc=000010 and
discards the width). The `prop_width_changes_only_sf` property is precisely
what distinguishes the correct REV16 from the broken REV32 — for REV32 the
analogous invariant fails because opc must swap with width.

**Note on negative contract:** a *trailing extra* operand (3 operands fed to a
2-operand encoder) is silently ignored because the function only indexes
`operands[0..2]`. This is benign and common in this assembler, so it is not
asserted as an error; only genuinely missing/mistyped operands are required to
return `Err`.
