# Bug: `dsb #N` encodes `DSB SY` instead of `DSB <CRm=N>`

**Law:** A numeric barrier operand `#N` (with `N` in `[0,15]`) is a valid
`DSB` input and must select CRm = N directly (matching the reference assembler).
The encoded CRm field must therefore equal the requested N, not default to
`0xF` (`sy`).

**Impact:** `dsb #7` is a *valid* `llvm-mc` input that encodes `dsb nsh`
(CRm = 7, `0xD503_379F`). The encoder instead emits `DSB SY` (`0xD503_3F9F`,
CRm = 15) — a *stronger* barrier than requested (SY is a full system barrier
whereas NSH is non-shareable), silently changing the program's memory-ordering
semantics for a valid input.

**Function:** `encode_dsb` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Differential oracle `llvm-mc-14 --triple=aarch64-linux-gnu`
(`dsb #7` → `dsb nsh`, CRm = 7).

**Minimal input:** `encode_dsb(&[Operand::Imm(7)])`.

**Expected:** `Ok(EncodeResult::Word(0xD503_379F))` — CRm = 7 (`nsh`).

**Actual:** `Ok(EncodeResult::Word(0xD503_3F9F))` — CRm = 15 (`sy`), because a
numeric `Operand::Imm` does not match the `Barrier`/`Symbol` arms and falls
through to the catch-all `_ => 0b1111`.

**Severity:** medium (silent wrong barrier for a valid input; inserts a stronger
barrier than intended, and a valid numeric CRm is mishandled).

**Regression test:** witness `b_s4b_dmb_dsb_numeric_operand_maps_to_crm` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s4b_dmb_dsb_numeric_operand_maps_to_crm -- --ignored
```
