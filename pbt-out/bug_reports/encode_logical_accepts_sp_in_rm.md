# Bug Report: `encode_logical` accepts SP in the Rm operand (silent SP→XZR aliasing)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_logical`
**Severity:** Correctness / assembler-conformance (silent mis-assembly)
**Pinned by:** `data_processing_div_fpsimd_sp_pbt::prop_logical_rejects_sp_in_rm`
(1 `#[ignore]`d witness; run with
`cargo test --lib data_processing_div_fpsimd_sp_pbt::prop_logical_rejects_sp_in_rm -- --ignored`)

## Summary

`encode_logical` resolves the Rm operand of the **logical (shifted register)**
form through `parse_reg_num`, which maps `"sp"`/`"wsp"` to register number
**31**. In the shifted-register encoding the **Rm** field is in the **Zr**
register class (field `31` decodes as **XZR/WZR**, *not* SP) — only **Rd** and
**Rn** permit SP for the non-flag-setting logical ops
(`ORR/AND/EOR <Xd|SP>, <Xn|SP>, <Xm>{, <shift>}`). The encoder therefore
**accepts** SP in the Rm position and silently emits an instruction that the
hardware reads as `ORR Xd, Xn, XZR` — the programmer's `sp` operand is dropped.

Root cause: `parse_reg_num` is register-class-agnostic (it cannot tell `sp`
from `xzr` — both yield `31`), and `encode_logical` performs no SP-vs-ZR
validation for the Rm field.

## Reproduction

```
input:   orr x0, x1, sp
actual:  Ok(Word(0xAA0103E0))   // decodes as  ORR X0, X1, XZR   (Rm=31 == XZR)
correct: Err("invalid operand: SP not permitted in Rm position")
```

Minimal failing input from the property: `use_wsp = false`
(i.e. `orr x0, x1, sp`). The same defect affects `and x0, x1, sp` and
`eor x0, x1, sp` (same code path, different `opc`).

## Spec basis (ARMv8-A ARM)

Logical (shifted register) operand classes:
- `Rd` → `Rt_SP` (SP permitted) for `AND/ORR/EOR/EON/ORN/BIC`; `Rt` (ZR only) for `ANDS/BICS`.
- `Rn` → `Rn_SP` (SP permitted).
- `Rm` → **`Rm` (ZR only)** — field `31` is `XZR`/`WZR`, never SP.

A conforming assembler (e.g. `clang --target=aarch64-linux-gnu`) rejects
`orr x0, x1, sp` with "invalid operand for instruction".

## Suggested fix

After `parse_reg_num(rm_name)`, reject `sp`/`wsp` for the Rm operand of the
shifted-register form (analogous to the existing `is_fp_reg` check that is
defined but unused for these paths). The non-flag-setting `Rd`/`Rn` SP
acceptance exercised by `prop_logical_accepts_sp_in_rd_rn_shifted` is
**correct** and must be preserved.

---

## Reconfirmations (independently witnessed in the same new module)

The new `data_processing_div_fpsimd_sp_pbt` module also independently
re-confirms two already-reported `encode_div` defects via additional
`#[ignore]`d witnesses (different module / generator, same root cause:
`get_reg` → `parse_reg_num` is register-class-agnostic):

| Witness | Finding | Prior report |
|---------|---------|--------------|
| `prop_div_rejects_fpsimd_registers` (min: `sdiv v0, x1, x2`) | `encode_div` accepts FP/SIMD registers (`d/s/q/v/h/b`) and encodes them as GP namesakes | `div_tst_cbz_register_class_and_sp_findings.md` |
| `prop_div_rejects_sp_operand` (min: `sdiv sp, x1, x2`) | `encode_div` accepts SP, silently aliasing it to XZR (data-processing (2 source) register class is Zr-only) | `encode_div_sp_operand_silently_accepted_as_xzr.md` |

These provide regression coverage independent of `div_tst_cbz_regclass_pbt`.

## Positive coverage (these PASS)

The same module documents the *correct* behaviour with passing properties:
- `prop_div_reference_and_fields` — SDIV/UDIV golden words
  (`sdiv x0,x0,x0=0x9AC00C00`, `udiv x0,x0,x0=0x9AC00800`, 32-bit variants) + field placement.
- `prop_div_sf_tracks_destination` — `sf` derived only from destination width.
- `prop_logical_accepts_sp_in_rd_rn_shifted` — SP is correctly accepted in Rd/Rn (field 31, sf=1).
- `logical_accepts_sp_in_immediate` — SP correctly accepted in the logical-immediate form
  (`orr sp, sp, #0xff` → Rd=Rn=31).
