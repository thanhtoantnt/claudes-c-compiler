# PBT Campaign: src/frontend/preprocessor/conditionals.rs

## Scan findings
- **Test layout:** Rust inline module tests (`#[cfg(test)] mod tests`) live in the source file. Existing tests in this repo follow that pattern; no separate `tests/` directory or Cargo test-path override was found.
- **Candidate modules:** `src/frontend/preprocessor/conditionals.rs` (`eval_const_expr` / expression tokenization and parsing)
- **Skipped modules:** (none)

## Module: src/frontend/preprocessor/conditionals.rs
- [x] Scan: identify targets — inline Rust tests confirmed in `conditionals.rs`
- [x] Plan: formalize properties — 3 properties approved in `pbt-out/PROPERTIES.md`
- [x] Test: write and run — inline proptest tests added and `cargo test eval_const_expr` passed
- [x] Review: triage results — no failing properties; no bugs found
