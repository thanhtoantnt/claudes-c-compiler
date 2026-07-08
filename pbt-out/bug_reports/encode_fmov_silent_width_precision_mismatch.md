# Bug Report: `encode_fmov` silently accepts width/precision-mismatched operands

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` — `encode_fmov`
**Severity:** Medium (incorrect-code emission, no diagnostic)
**All 5 PBT properties pass** — this report documents a *separate* functional gap
found during analysis and confirmed by probe.

## Summary

`encode_fmov` derives the instruction's `sf`/`ftype` from **only one** operand of
each pair and never cross-checks that the two operands have matching width
(GP↔FP) or matching precision (FP↔FP). As a result, operands that the AArch64
ARM defines as UNDEFINED/UNALLOCATED are accepted and encoded as a different,
plausible-looking instruction with **no error**. The wrong encoding is then
written verbatim into the object file.

## Root cause

- **GP→FP branch** (`rd_is_fp && !rm_is_fp`): `is_double = rd_lower.starts_with('d')`.
  The GP source register's width (`x` vs `w`) is **ignored**.
- **FP→GP branch** (`!rd_is_fp && rm_is_fp`): `is_double = rm_lower.starts_with('d')`.
  The GP destination's width (`x` vs `w`) is **ignored**.
- **FP↔FP branch** (`rd_is_fp && rm_is_fp`): `is_double = rd.starts_with('d') || rm.starts_with('d')`.
  No check that both operands are the same precision (`s` vs `d`).

## Confirmed cases (probe output, this build)

| Input          | Emitted word    | Actually decodes as | Correct behavior |
|----------------|-----------------|---------------------|------------------|
| `fmov d0, w1`  | `0x9E670020` (sf=1, ftype=01) | `fmov d0, x1` | **Err** — D dest needs X source |
| `fmov s0, x1`  | `0x1E270020` (sf=0, ftype=00) | `fmov s0, w1` | **Err** — S dest needs W source |
| `fmov w0, d1`  | `0x9E660020` (sf=1, ftype=01) | `fmov x0, d1` | **Err** — D source needs X dest |
| `fmov x0, s1`  | `0x1E260020` (sf=0, ftype=00) | `fmov w0, s1` | **Err** — S source needs W dest |
| `fmov d0, s1`  | `0x1E604020` (ftype=01)        | `fmov d0, d1` | **Err** — FP↔FP requires matching precision |

Each emits a syntactically valid 32-bit word that the disassembler renders as a
*different instruction* than the one written, with the source/dest register
number stolen from the wrong-width/wrong-precision operand.

## Impact

- A typo or a buggy codegen pass (e.g. passing a W source to a D move, or an S
  register paired with a D) produces a **wrong-width register access** with no
  assembler error, silently corrupting codegen. Hard to diagnose downstream.
- `fmov d0, s1` reinterprets the S register's number in a D-context encoding —
  architecturally UNALLOCATED per the ARMv8 ARM (FMOV (register) requires
  `ftype` to match both operand sizes).

## Suggested fix

Validate consistency before encoding:
- GP→FP: `Dd ↔ Xn`, `Sd ↔ Wn` (reject `Dd,Wn` and `Sd,Xn`).
- FP→GP: `Xd ↔ Dn`, `Wd ↔ Sn` (reject `Xd,Sn` and `Wd,Dn`).
- FP↔FP: require both operands the same precision (`s/s` or `d/d`); reject mixed.

Return `Err("fmov: operand width/precision mismatch")` on violation. (Note also
that the GP-register width helpers `is_64bit_reg`/`is_32bit_reg` already exist in
`encoder/mod.rs` and can be reused.)

## Test coverage note

The added property suite (`fp_scalar::tests`, 5 properties) proves the **happy
path is bit-exact** (reference encodings `0x1E204020`/`0x1E604020`/`0x9E670000`/
`0x1E270000`/`0x9E660000`/`0x1E260000`, correct `sf`/`ftype`/`rmode`, full 5-bit
register round-trip with **no truncation** for `0..=31`, and rejection of
immediates / short arity / out-of-range register numbers). The mismatch gap
above is deliberately *not* asserted as a failing test to keep the suite green;
it is tracked here.
