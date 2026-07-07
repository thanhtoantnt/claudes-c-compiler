# PBT Campaign: src/frontend/lexer/scan.rs

## Scan findings
- **Test layout:** Rust inline module tests (`#[cfg(test)] mod tests`) live inside the source file; existing tests in this repo follow that pattern. No separate `tests/` directory or Cargo test-path override was found.
- **Candidate modules:** `src/frontend/lexer/scan.rs` (`Lexer::tokenize` / `next_token` / skip logic / GNU keyword mode)
- **Skipped modules:** (none)

## Module: src/frontend/lexer/scan.rs
- [x] Scan: identify targets
- [x] Plan: formalize properties
- [x] Test: write and run
- [x] Review: triage results
