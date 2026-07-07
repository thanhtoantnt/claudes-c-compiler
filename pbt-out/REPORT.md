# PBT Campaign Report: `encode_ldur_stur`

## Summary

**Date:** 2026-07-08
**Repository:** `/home/toan/evaluation/claudes-c-compiler`
**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldur_stur`
**Tests added:** 6
**Result:** 5 passing, 1 failing property, 1 confirmed bug

## Modules Tested

| Module | Tests | Bugs | Oracles Used |
|--------|-------|------|--------------|
| `src/backend/arm/assembler/encoder/load_store.rs` (`encode_ldur_stur`) | 6 | 1 | reference-encoding (ARM-ARM golden), field-placement, differential (load⊕store), negative/error contract |

## Bugs Found

**`encode_ldur_stur` silently wraps out-of-range `imm9` instead of rejecting it (silent mis-assembly).**

- The 9-bit `imm9` field of LDUR/STUR/LDTR/STTR is a **signed** immediate with encodable range **[-256, +255]** (ARMv8-A ARM §C4.1.66). The implementation does `imm9_enc = (imm9 as u32) & 0x1FF` and always returns `Ok`, with no range check.
- Witness from the shrunk failing property (`prop_out_of_range_imm9_is_rejected`): `offset = 256` (excess=1, negative=false).
- Effect: `ldur x0, [x1, #256]` silently encodes as `0xF8500020`, which is actually `ldur x0, [x1, #-256]` (256 wraps to 9-bit field value 256 = signed −256). `#-257` wraps to `#255` (sign flip); `#512` wraps to `#0`.
- Expected behavior: return `Err` (the programmer should use the `LDR` unsigned-offset form, which *does* reject out-of-range immediates).
- Contrast: the sibling `encode_ldr_str` in the same file correctly rejects out-of-range immediates (its negative-contract property passes); `encode_ldur_stur` is the inconsistent, unsafe variant.
- Reproducer: `cargo test --lib prop_encode_ldur_stur`
- Suggested fix: `if imm9 < -256 || imm9 > 255 { return Err(format!("ldur/stur: unscaled offset {} out of range [-256, 255]", imm9)); }` before the masking step.
- Full write-up: `pbt-out/encode_ldur_stur_bug_report.md`

## Design Caveats

**None.** No documented intentional behavior required a caveat.

Every questionable behavior in `encode_ldur_stur` was investigated against the
available evidence (the sole docstring, `load_store.rs:247-248`, which documents
only the encoding *format* `size 111 V 00 opc 0 imm9 00 Rn Rt`) and resolved one
of two ways:

- **Verified correct** (no caveat needed) — field layout, sign-extension of
  in-range `imm9`, size/opc derivation, and the load⊕store opc-bit differential
  are all confirmed by the five passing properties.
- **Reclassified as a bug** (see ## Bugs Found above) — the `imm9`
  out-of-range wrapping has **no** doc evidence of intentionality: no docstring,
  no `// Note:`/`// TODO:` comment, no spec, and no existing test asserts that
  wrapping is desired. By the bug-by-default rule it is filed as a defect, not
  a design choice. (For contrast, `encode_ldnp_stnp` at `load_store.rs:517`
  *does* document a deliberate limitation via `// TODO: Only handles integer
  registers (V=0)...`; `encode_ldur_stur` has no such annotation.)

Testing-methodology notes (golden-encoding anchoring to the ARMv8-A ARM, the
`op2_bits` ∈ {00, 10} coverage, and the load⊕store differential oracle) are not
behavioral caveats and are therefore not listed here.

## Test Files Created

| File | Tests Added |
|------|-------------|
| `src/backend/arm/assembler/encoder/load_store.rs` (module `prop_encode_ldur_stur_tests`) | 6 |

## Output Directories

- `pbt-out/`
- `proptest-regressions/` (failure persisted at `proptest-regressions/backend/arm/assembler/encoder/load_store.txt`)
