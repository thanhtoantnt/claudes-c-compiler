# Bug: encode_ic / encode_tlbi accept a spurious register on no-register ops

**Law:** `IC IALLUIS` / `IC IALLU` and the "all" `TLBI` ops (vmalle1, vmalle1is,
alle1, alle1is, alle2is, vmalls12e1, vmalls12e1is) take NO register; the `Rt`
field must be `0b11111` (XZR). Supplying a register operand must be an error.
**Impact:** `ic iallu, x5` silently overwrites the mandatory `Rt = XZR` field with
5, and `tlbi vmalle1, x5` likewise emits an illegal encoding. clang/LLVM reject
these.
**Function:** `encode_ic` (IALLUIS/IALLU), `encode_tlbi` (no-reg ops) —
`src/backend/arm/assembler/encoder/system.rs`.
**Detected by:** Negative/Error contract (property-based).
**Witnesses (all `#[ignore]`, run with `cargo test --lib system_sys_at_dc_ic_tlbi -- --ignored`):**
- `witness_ic_no_reg_op_rejects_register` — `encode_ic("ialluis, x0")`, minimal `op_idx=0, rt=0`
- `witness_tlbi_no_reg_op_rejects_register` — `encode_tlbi(&[], "vmalle1is, x0")`, minimal `op_idx=0, rt=0`
**Expected:** `Err` (clang-14: *"specified ic/tlbi op does not use a register"*).
Even an explicit `xzr` is rejected: clang rejects `tlbi vmalle1, xzr`.
**Actual:** `Ok(…)` with the base word's low 5 bits overwritten by the register
number (e.g. `ic iallu, x5` → `0xD508_7515` instead of `0xD508_751F`).
**Root cause:** `(base & !0x1F) | rt` unconditionally ORs the provided register
into the Rt field even for instructions whose Rt is architecturally fixed to XZR.
**Severity:** medium
**Regression test:** `src/backend/arm/assembler/encoder/system_sys_at_dc_ic_tlbi_pbt.rs`
(witness properties, run with `--ignored`).
**Oracle verification:** clang-14 rejects `ic iallu, x5`, `ic ialluis, x0`,
`tlbi vmalle1, x5`, `tlbi vmalle1is, x3`, `tlbi vmalle1, xzr` — all
*"does not use a register"*.
