# Bug: encode_sys silently masks out-of-range op1/CRn/CRm/op2 fields

**Law:** `SYS #op1, Cn, Cm, #op2` requires `op1,op2 ∈ [0,7]` and
`CRn,CRm ∈ [0,15]` (ARMv8 ARM §C5.2). Out-of-range values must be rejected.
**Impact:** A typo like `sys #8, c0, c0, #0` is silently accepted and aliases a
different (valid) instruction (`op1 & 7 == 0`), producing wrong machine code with
no diagnostic. Mis-assembled system-register accesses can corrupt EL state.
**Function:** `encode_sys` (`src/backend/arm/assembler/encoder/system.rs`)
**Detected by:** Negative/Error contract (property-based).
**Witnesses (all `#[ignore]`, run with `cargo test --lib system_sys_at_dc_ic_tlbi -- --ignored`):**
- `witness_sys_rejects_out_of_range_op1` — minimal input `op1 = 8`
- `witness_sys_rejects_out_of_range_op2` — minimal input `op2 = 8`
- `witness_sys_rejects_out_of_range_crn_crm` — minimal input `crn = 16, crm = 16`
**Minimal input:** `encode_sys("#8, c0, c0, #0")`
**Expected:** `Err` (clang-14: *"immediate must be an integer in range [0, 7]."*;
for CRn: *"Expected cN operand where 0 <= N <= 15"*)
**Actual:** `Ok(0xD508_0000)` — `8 & 7 == 0`, so it aliases `sys #0, c0, c0, #0`.
**Root cause:** `((op1 & 7) << 16) | ((crn & 0xF) << 12) | ((crm & 0xF) << 8) | ((op2 & 7) << 5)`
masks instead of range-checking before `parse()`.
**Severity:** medium
**Regression test:** `src/backend/arm/assembler/encoder/system_sys_at_dc_ic_tlbi_pbt.rs`
(witness properties, run with `--ignored`).
**Oracle verification:** clang-14 `--target=aarch64-linux-gnu` rejects `sys #8, …`,
`sys #0, c16, c0, #0`, `sys #0, c0, c0, #8`.
