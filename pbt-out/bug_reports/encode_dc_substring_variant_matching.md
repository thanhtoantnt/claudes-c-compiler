# Bug: encode_dc matches variant names by substring

**Law:** A `DC` variant name must match one of the supported operations exactly
(civac/cvac/cvap/cvau/ivac/zva). Unknown variant names must be rejected.
**Impact:** `dc xcvacx, x0` is silently accepted and aliased to `DC CVAC`
because `op.contains("cvac")` is true. A typo'd or fabricated variant produces a
real cache instruction with no diagnostic.
**Function:** `encode_dc` (`src/backend/arm/assembler/encoder/system.rs`).
**Detected by:** Negative/Error contract (property-based).
**Witness (`#[ignore]`, run with `cargo test --lib system_sys_at_dc_ic_tlbi -- --ignored`):**
- `witness_dc_rejects_bogus_substring_variant` — minimal input `v_idx=0` → variant `xcvacx`
**Minimal input:** `encode_dc(&[Operand::Symbol("xcvacx"), Operand::Reg("x0")], "xcvacx, x0")`
**Expected:** `Err` (clang-14: *"invalid operand for DC instruction"*).
**Actual:** `Ok(0xD50B_7A20)` — the CVAC encoding (because `"xcvacx".contains("cvac")`).
**Root cause:** the dispatcher uses `op.contains("cvac")` / `contains("zva")` / …
instead of exact equality.
**Severity:** low
**Regression test:** `src/backend/arm/assembler/encoder/system_sys_at_dc_ic_tlbi_pbt.rs`
(witness property, run with `--ignored`). The passing property
`dc_rejects_unknown_variant` separately confirms genuinely-unknown names (those
containing no DC substring) are correctly rejected.
**Oracle verification:** clang-14 rejects `dc xcvacx, x0`
(*"invalid operand for DC instruction"*).
