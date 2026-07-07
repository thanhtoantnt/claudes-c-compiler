# PBT Bug Summary: claudes-c-compiler

**Project:** [anthropics/claudes-c-compiler](https://github.com/anthropics/claudes-c-compiler)
**Date:** 2026-07-07
**Method:** Property-based testing (proptest) against the real compiled Rust source.

## Bugs Found: 2

### 1. `f64_to_f128_bytes_lossless` panics on subnormal f64 values

- **File:** `src/common/long_double.rs:1040`
- **Severity:** Medium
- **Witness:** `val = 5.45247436838069e-309` (subnormal)
- **Symptom:** Panic in debug mode (unsigned subtraction overflow); corrupt f128 encoding in release mode.
- **Root cause:** The function computes `biased_exp - 1023` without checking for subnormals (`biased_exp == 0`). Subnormal f64 values require renormalization before computing the f128 exponent.
- **Report:** [`pbt-out/bug_reports/f64_to_f128_subnormal_panic.md`](bug_reports/f64_to_f128_subnormal_panic.md)

### 2. `f64_to_x87_bytes_simple` silently loses subnormal f64 values

- **File:** `src/common/long_double.rs` (x87 encoder)
- **Severity:** Medium
- **Witness:** `val = 5.45247436838069e-309` (subnormal)
- **Symptom:** Encodes to x87 bytes that decode back as `0.0` — the subnormal value is silently dropped.
- **Root cause:** The x87 encoder or decoder does not correctly handle the subnormal→x87 representation (likely missing the explicit J-bit or mis-mapping the x87 exponent for denormals).
- **Report:** [`pbt-out/bug_reports/f64_to_x87_subnormal_loss.md`](bug_reports/f64_to_x87_subnormal_loss.md)

## Properties Written: 7

| Property | File | Oracle | Result |
|---|---|---|---|
| `f64_to_f128_roundtrip` | `src/common/long_double_pbt.rs` | Round-trip (4a) | **FAIL** — panic on subnormal |
| `f64_to_x87_roundtrip` | `src/common/long_double_pbt.rs` | Round-trip (4a) | **FAIL** — subnormal → 0.0 |
| `f64_zero_sign_preserved_in_f128` | `src/common/long_double_pbt.rs` | Invariant (4d) | PASS |
| `f64_special_values_survive_f128_roundtrip` | `src/common/long_double_pbt.rs` | Negative/Error (4e) | PASS |
| `tokenize_metamorphic` | `src/frontend/lexer/scan.rs` | Metamorphic | PASS |
| `set_gnu_extensions_reference` | `src/frontend/lexer/scan.rs` | Reference | PASS |
| `tokenize_invariant` | `src/frontend/lexer/scan.rs` | Invariant (4d) | PASS |

## Reproduction

```sh
cd ~/evaluation/claudes-c-compiler
cargo test --lib common::long_double_pbt    # 2 FAIL (the bugs)
cargo test --lib frontend::lexer::scan      # 3 PASS (lexer is correct)
```

## Notes

- Both bugs are in the **same class**: subnormal (denormalized) floating-point values. LLM-generated code correctly handles normal values, zeros, infinities, and NaNs — but misses subnormals, which are the rarest edge case in IEEE 754.
- The bugs were found by a 4-property manual PBT suite in **< 1 second** of test execution (proptest shrunk to the minimal witness immediately).
- pi-pbt's autonomous run on the lexer produced 3 well-designed properties that all pass — the lexer is correct for the tested surfaces. The float-conversion bugs required manual PBT targeting (pi-pbt did not autonomously scan `long_double.rs`).
