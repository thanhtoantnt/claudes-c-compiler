# Bug: `dmb #N` encodes `DMB SY` instead of `DMB <CRm=N>`

**Law:** A numeric barrier operand `#N` (with `N` in `[0,15]`) is a valid
`DMB` input and must select CRm = N directly (matching the reference assembler).
The encoded CRm field must therefore equal the requested N, not default to
`0xF` (`sy`).

**Impact:** `dmb #5` is a *valid* `llvm-mc` input that encodes `dmb nshld`
(CRm = 5, `0xD503_35BF`). The encoder instead emits `DMB SY` (`0xD503_3FBF`,
CRm = 15) — a *different* barrier than the one a valid input requested. Silent
wrong instruction for a valid input.

**Function:** `encode_dmb` — `src/backend/arm/assembler/encoder/system.rs`

**Detected by:** Differential oracle `llvm-mc-14 --triple=aarch64-linux-gnu`
(`dmb #5` → `dmb nshld`, CRm = 5).

**Minimal input:** `encode_dmb(&[Operand::Imm(5)])`.

**Expected:** `Ok(EncodeResult::Word(0xD503_35BF))` — CRm = 5 (`nshld`).

**Actual:** `Ok(EncodeResult::Word(0xD503_3FBF))` — CRm = 15 (`sy`), because a
numeric `Operand::Imm` does not match the `Barrier`/`Symbol` arms and falls
through to the catch-all `_ => 0b1111`.

**Severity:** medium (silent wrong barrier for a valid input; weakens/removes
the intended memory-ordering guarantee — invisible until a rare concurrency
failure).

**Regression test:** witness `b_s4b_dmb_dsb_numeric_operand_maps_to_crm` in
`src/backend/arm/assembler/encoder/system_barriers_hints_pbt.rs` (marked
`#[ignore]`). Run with:
```
cargo test --lib system_barriers_hints::b_s4b_dmb_dsb_numeric_operand_maps_to_crm -- --ignored
```
