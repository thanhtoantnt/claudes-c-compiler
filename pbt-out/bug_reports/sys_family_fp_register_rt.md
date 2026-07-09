# Bug: SYS-family encoders accept FP/SIMD registers as Rt

**Law:** The `Rt` operand of `SYS`, `AT`, `IC`, `TLBI`, and `DC` must be a
general-purpose (X) register. FP/SIMD register names (`d0`, `s1`, `q5`, `v0`, …)
are not legal `Rt` operands.
**Impact:** `at s1e1r, d0` silently encodes with `Rt = parse_reg_num("d0") = 0`,
producing an illegal instruction word that clang/LLVM reject. Downstream code may
emit machine code that faults or targets the wrong register.
**Function:** `encode_sys`, `encode_at`, `encode_ic`, `encode_tlbi`, `encode_dc`
(all in `src/backend/arm/assembler/encoder/system.rs`).
**Detected by:** Negative/Error contract (property-based).
**Common root cause:** all five parse the register with the shared helper
`parse_reg_num` (`encoder/mod.rs`), which accepts `d/s/q/v/h/b` prefixes. This is
the same root cause as the already-filed `encode_mrs_msr_accept_fp_register_as_rt.md`
and `parse_reg_num_permissive_spelling.md`; this report records that the contract is
also violated by the SYS-family encoders. (Consolidated by shared root cause; each
function has its own witness below.)
**Witnesses (all `#[ignore]`, run with `cargo test --lib system_sys_at_dc_ic_tlbi -- --ignored`):**
- `witness_sys_rejects_fp_register` — `encode_sys("#0, c0, c0, #0, d0")`, fp_idx=0 → `d0`
- `witness_at_rejects_fp_register` — `encode_at(&[], "s1e1r, d0")`
- `witness_ic_rejects_fp_register` — `encode_ic("ivau, d0")`
- `witness_tlbi_rejects_fp_register` — `encode_tlbi(&[], "vae1is, d0")`
- `witness_dc_rejects_fp_register` — `encode_dc(&[Symbol("cvac"), Reg("d0")], "cvac, d0")`
**Expected:** `Err` (clang-14: *"invalid operand for instruction"*).
**Actual:** `Ok(…)` with `Rt` set to the FP lane number.
**Severity:** medium
**Regression test:** `src/backend/arm/assembler/encoder/system_sys_at_dc_ic_tlbi_pbt.rs`
(witness properties, run with `--ignored`).
**Oracle verification:** clang-14 rejects `sys #0, c0, c0, #0, d0`,
`at s1e1r, d0`, `ic ivau, d0`, `tlbi vae1, d0`, `dc cvac, d0` — all
*"invalid operand for instruction"*.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/321
