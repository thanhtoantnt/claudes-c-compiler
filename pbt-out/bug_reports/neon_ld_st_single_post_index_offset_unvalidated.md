# `encode_neon_ld_st_single` — post-index immediate is silently discarded

- **Module:** `src/backend/arm/assembler/encoder/neon.rs`
- **Function:** `encode_neon_ld_st_single`
- **Witness test:** `src/backend/arm/assembler/encoder/neon_ld_st_single_pbt.rs` — `post_index_offset_is_validated` (`#[ignore]`d `proptest!`; default `cargo test` stays green)
- **Reproduce:** `cargo test --lib neon_ld_st_single_pbt -- --ignored post_index_offset_is_validated`
- **Shrunk counterexample (Falsifiable):** `elem="b", num=1, bad_offset=2` ⇒ `st1 {v0.b}[0], [x1], #2` (correct is `#1`) accepted and emitted as the correct word, not rejected

## Symptom

A single-structure post-index load/store with a **wrong** `#imm` is accepted
and produces the same word as the correct immediate.

```
st2 {v0.h, v1.h}[0], [x1], #4   → 0x0DBF4020   (correct imm = 2 regs × 2 bytes)
st2 {v0.h, v1.h}[0], [x1], #99  → 0x0DBF4020   (wrong imm, NOT rejected)
st1 {v0.b}[0], [x1], #2         → emitted (wrong imm #2 vs correct #1, NOT rejected)
```

## Root cause

Both post-index code paths hard-code `Rm = 0b11111` (which the ARM ARM defines
as "immediate post-index by the element transfer size") and never inspect the
supplied offset:

```rust
if let Some(_offset) = post_index {
    // ... Rm = 0b11111, `_offset` is bound then dropped ...
}
```

Per the ARMv8-A ARM, the post-index immediate **must equal** the number of
bytes transferred (`num_structs * sizeof(element)`); any other value is
unpredictable / should be rejected by the assembler. This is the same defect
class as the already-documented `LD1R` offset bug
(`LD1R_OFFSET_BUG_REPORT.md`).

## Suggested fix

Validate that the post-index offset equals `num_structs * element_size_bytes`
before emitting, returning `Err` otherwise.

## Caveat

The `num_structs` and element size are both known to the encoder, so the
required transfer-size check is straightforward; the fix is purely additive
(input validation) and does not change the encoding of any currently-correct
input.
