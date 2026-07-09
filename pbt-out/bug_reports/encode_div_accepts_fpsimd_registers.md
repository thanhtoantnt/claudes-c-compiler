# Bug Report: `encode_div` accepts FP/SIMD registers (`d/s/q/v/h/b`) and encodes them as GP namesakes

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_div`
**Severity:** Correctness / assembler-conformance (silent mis-assembly)
**Pinned by:** `data_processing_div_fpsimd_sp_pbt::prop_div_rejects_fpsimd_registers`
(1 `#[ignore]`d witness; run with
`cargo test --lib data_processing_div_fpsimd_sp_pbt::prop_div_rejects_fpsimd_registers -- --ignored`)

## Summary

`encode_div` (which emits `SDIV`/`UDIV`) resolves all three operands through
the shared `get_reg` → `parse_reg_num` path. `parse_reg_num` is
**register-class-agnostic**: for every FP/SIMD prefix (`d`, `s`, `q`, `v`, `h`,
`b`) it returns the trailing number, and `is_64bit_reg` returns `false` for all
of them (none starts with `x`). The helper `is_fp_reg` (which *would* catch
this) is defined in `encoder/mod.rs` but is **never called** by `encode_div`.

The result: `SDIV`/`UDIV` — which are defined **only** for general-purpose
registers (`<Wd>,<Wn>,<Wm>` / `<Xd>,<Xn>,<Xm>`, the integer "data-processing
(2 source)" class) — silently accept FP/SIMD operands and encode them verbatim
as their GP namesakes. `sdiv v0, x1, x2` is accepted and emitted as a 32-bit
SDIV with `Rd=0`; the programmer's vector/FP register is dropped.

Root cause: `get_reg`/`parse_reg_num` cannot distinguish register banks, and
`encode_div` performs no FP/SIMD-bank validation before emitting.

## Reproduction

```
input:   sdiv v0, x1, x2          (FP/SIMD register v0 in destination)
actual:  Ok(Word(0x1AC20C20))     // decodes as  SDIV W0, X1, X2   (Rd=0, 32-bit)
correct: Err("invalid operand: FP/SIMD register v0 not permitted for SDIV/UDIV")
```

Minimal failing input from the property: `prefix = "v", n = 0, bad_pos = 0,
unsigned = false` → `sdiv v0, x1, x2`. The witness also fails for every other
FP/SIMD prefix (`d`, `s`, `q`, `h`, `b`), every register number `0..=31`, every
operand position (`0..3`, i.e. destination `Rd`, source `Rn`, source `Rm`),
and for both `SDIV` (`unsigned=false`) and `UDIV` (`unsigned=true`).

Concretely, all of the following are wrongly accepted:
- `sdiv d0, x1, x2` → encoded as `sdiv w0, x1, x2` (Rd=0)
- `udiv x0, v1, x2` → encoded as `udiv x0, x1, x2` (Rn=1)
- `sdiv x0, x1, q2` → encoded as `sdiv x0, x1, x2` (Rm=2)

## Spec basis (ARMv8-A ARM)

`SDIV` and `UDIV` belong to the **Data-processing (2 source)** encoding group,
which is defined exclusively for the general-purpose register file:

```
  SDIV <Wd>, <Wn>, <Wm>     ; sf=0
  SDIV <Xd>, <Xn>, <Xm>     ; sf=1
  UDIV <Wd>, <Wn>, <Wm>
  UDIV <Xd>, <Xn>, <Xm>
```

There is **no** FP/SIMD form. A conforming assembler
(`clang --target=aarch64-linux-gnu`) rejects every case above with
"invalid operand for instruction".

## Suggested fix

After resolving each operand, reject names whose prefix is one of
`d`/`s`/`q`/`v`/`h`/`b` (the existing `is_fp_reg` helper already implements
this check — it just needs to be applied to `Rd`, `Rn`, and `Rm` in
`encode_div`). Return an `Err` describing the rejected register bank.

## Related reports (distinct findings, not deduped)

- `encode_div_sp_operand_silently_accepted_as_xzr.md` — `encode_div` also
  accepts `sp`/`wsp`, silently aliasing them to XZR (Zr-only register class).
  Pinned by `prop_div_rejects_sp_operand` in the same new module.
- `encode_div_silent_width_acceptance.md` — `encode_div` accepts mismatched
  W/X widths (separate, already-reported finding).
- `div_tst_cbz_register_class_and_sp_findings.md` — an earlier *combined*
  report that previously lumped this `encode_div` FP/SIMD finding together
  with the `encode_tst`/`encode_cbz` families. This file now provides the
  dedicated, per-function report for `encode_div` as required.
