# Bug: SYS-family encoders silently default a missing required Rt register

**Law:** `AT <op>`, `IC IVAU`, the register-taking `TLBI` ops (vae1, ipas2e1,
aside1is, …), and all `DC` variants REQUIRE an `Xt` register operand. Omitting it
must be an error.
**Impact:** A malformed `dc cvac` (no register) silently encodes with `Rt = 0`
(x0) instead of erroring; `at s1e1r` / `ic ivau` / `tlbi vae1` silently encode with
`Rt = XZR` (31). The assembler emits wrong/no-op code with no diagnostic.
**Function:** `encode_at`, `encode_ic` (IVAU), `encode_tlbi` (reg-ops),
`encode_dc` — `src/backend/arm/assembler/encoder/system.rs`.
**Detected by:** Negative/Error contract (property-based).
**Witnesses (all `#[ignore]`, run with `cargo test --lib system_sys_at_dc_ic_tlbi -- --ignored`):**
- `witness_at_rejects_missing_register` — `encode_at(&[], "s1e1r")` → defaults Rt=31
- `witness_ic_ivau_rejects_missing_register` — `encode_ic("ivau")` → defaults Rt=31
- `witness_tlbi_reg_op_rejects_missing_register` — `encode_tlbi(&[], "vae1is")` → defaults Rt=31
- `witness_dc_rejects_missing_register` — `encode_dc(&[Symbol("cvac")], "cvac")` → defaults Rt=**0** (x0)
**Expected:** `Err` (clang-14: *"specified at/ic/tlbi/dc op requires a register"*).
**Actual:** `Ok(…)` with a silently defaulted `Rt`.
**Root cause:** the no-operand branch of each encoder falls back to a default `Rt`
(AT/IC/TLBI → 31/XZR, DC → 0/x0) instead of returning `Err`.
**Severity:** medium
**Regression test:** `src/backend/arm/assembler/encoder/system_sys_at_dc_ic_tlbi_pbt.rs`
(witness properties, run with `--ignored`).
**Oracle verification:** clang-14 rejects `at s1e1r`, `ic ivau`, `tlbi vae1`,
`dc cvac` — all *"requires a register"*.
