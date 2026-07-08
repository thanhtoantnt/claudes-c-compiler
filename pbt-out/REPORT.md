# REPORT

## Summary

Function-scoped property-based testing campaign on `encode_smull` in
`src/backend/arm/assembler/encoder/data_processing.rs`. Added 6 `proptest!`
properties to the module's existing `mod tests`: a reference-constant oracle,
fixed-field placement, deterministic sf behavior, three negative contracts, and a
width-validation negative contract. Five pass; **one fails** (`smull_rejects_wrong_width_destination`),
confirming a real SUT violation with a shrunk counterexample. The failure is
reported as B1 below.

Methodology: the reference constant `0x9B207C00` was derived independently from the
ARMv8 ARM SMADDL bit-string `1 00 11011 001 Rm 0 11111 Rn Rd` with Rm=Rn=Rd=0 and
OR'd with the register field placements (not copied from the implementation).
Register-number generators are scoped to `0..=30` (excluding 31) so `WZR`/`XZR`/
`SP`/`WSP` aliasing is exercised only by the dedicated negative-contract
properties. proptest default shrinking was sufficient; no custom `Strategy` types
were needed beyond the existing `xreg`/`wreg` helpers.

## Modules Tested

| Module (file) | Function | Oracle type | Properties added | Status |
|---|---|---|---|---|
| `src/backend/arm/assembler/encoder/data_processing.rs` | `encode_smull` | reference constant (spec bit-string) + field placement + negative contract | 6 | 5 pass, 1 FAILS (B1) |

## Bugs Found

### B1 — `encode_smull` accepts a 32-bit (W) destination, silently miscoding as a 64-bit result

**Witness (failing, shrunk PBT property):**
- Property: `smull_rejects_wrong_width_destination`
  (`src/backend/arm/assembler/encoder/data_processing.rs`, `mod tests`)
- **minimal failing input:** `n = 0`
- **Counterexample operands:** `[wreg(0), wreg(0), wreg(0)]` ≡ `smull w0, w0, w0`
- **Actual:** `encode_smull(...)` returns `Ok(EncodeResult::Word(0x9B207C00))`,
  i.e. emits `SMADDL X0, W0, W0, XZR` — a **64-bit** destination write.
- **Expected:** `Err` (SMULL has no valid encoding with a `W` destination).
- **Run record:** `successes: 0`, `local rejects: 0` — failed on first draw,
  shrunk to `n = 0`.
- **Reproduce:** `cargo test --lib smull_rejects_wrong_width_destination`
  → `assertion failed: encode_smull(&ops).is_err()`

**Spec / doc evidence (oracle anchor):**
- ARMv8 ARM: `SMULL <Xd>, <Wn>, <Wm>` is the alias of `SMADDL <Xd>, <Wn>, <Wm>, <XZR>`;
  the destination MUST be a 64-bit (X) register.
- In-tree docstring on `encode_smull`:
  `SMULL Xd, Wn, Wm -> SMADDL Xd, Wn, Wm, XZR`.

**Root cause:** `get_reg` (defined at `src/backend/arm/assembler/encoder/mod.rs:956`)
returns `(num, is_64)`, but `encode_smull` binds all three `is_64` flags to `_` and
hardcodes `(1u32 << 31)` as `sf`. The destination width is never validated, so a `W`
destination is silently re-encoded as `X`.

**Impact:** valid-looking ARM word (`0x9B207C00`) that mismatches the assembly text —
silent miscompilation of width-incorrect source. Severity: medium. The fixed-bit
layout itself is correct (verified by the passing `smull_reference_encoding` and
`smull_field_placement` properties); the defect is the missing width validation.

**Suggested fix:** bind `(rd, rd_is_64)`, `(rn, rn_is_64)`, `(rm, rm_is_64)` from
`get_reg`; return `Err` unless `rd_is_64 && !rn_is_64 && !rm_is_64`. The same
`is_64`-discarding pattern appears in `encode_umull`, `encode_smaddl`,
`encode_umaddl`, `encode_smulh`, `encode_umulh` — worth auditing together. Full
write-up in `pbt-out/SMULL_BUG_REPORT.md`.

## Design Caveats

None.

## Test Files Created

No new test files were created. The 6 properties were added to the existing
`#[cfg(test)] mod tests` block inside
`src/backend/arm/assembler/encoder/data_processing.rs`, immediately after the
shared `expect_word` helper, reusing the module's existing field extractors
(`sf_of`, `rm_of`, `rn_of`, `rd_of`) and `xreg`/`wreg` builders. Properties added:

- `smull_reference_encoding` (pass)
- `smull_field_placement` (pass)
- `smull_sf_always_set_regardless_of_source_width` (pass)
- `smull_rejects_too_few_operands` (pass)
- `smull_rejects_non_register_operands` (pass)
- `smull_rejects_wrong_width_destination` (**FAILS — witness for B1**)

## Output Directories

- `pbt-out/` — this report.
- `pbt-out/SMULL_BUG_REPORT.md` — full bug write-up for B1 (root cause, witness,
  suggested fix, list of sibling functions with the same pattern).

This was an in-tree, function-scoped campaign that edits the module's own test
block, so no `pbt-out/tests/` generated-test directory was produced.
