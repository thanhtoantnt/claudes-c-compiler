# Bug: `encode_cnt` silently accepts undefined NEON arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs::encode_cnt`
**Tests:** `src/backend/arm/assembler/encoder/neon_cnt_pbt.rs`

## Summary
The NEON `CNT` encoder validates neither the destination nor the source
register arrangement. Per the ARMv8 ARM, `CNT` is defined **only** for
`.8b` (Q=0) and `.16b` (Q=1); every other arrangement is UNDEFINED.
The implementation tests only `arr_d == "16b"` and falls through to `Q=0`
for *anything* else, silently emitting a `0x0E205800`-shaped word.

## Root cause
```rust
let q: u32 = if arr_d == "16b" { 1 } else { 0 }; // .8b -> Q=0, .16b -> Q=1
```
No check that `arr_d` is one of `{"8b", "16b"}` (and the source arrangement
`_arr_n` is read then discarded entirely — see `prop_cnt_source_arrangement_ignored`).

## Demonstrated failures
- `prop_cnt_rejects_non_byte_arrangements` — minimal failing input
  `CNT v0.4h, v0.8b` returns `Ok(Word(0x0E205800))` (identical to
  `CNT v0.8b,v0.8b`) instead of `Err`. Same for `.8h`, `.2s`, `.4s`, `.1d`,
  `.2d`, and for a bare `Operand::Reg` (empty arrangement) as destination.
- `encode_cnt(&[vreg_arr(0,"4h"), vreg_arr(0,"8b")])` => `Ok(Word(0x0E205800))`.

## Impact
`cnt v0.4h, v0.8b` (a typo / invalid form) produces a syntactically
valid-looking `0x0E205800` = `cnt v0.8b, v0.8b` with no diagnostic. The user
is silently given *different semantics* than written.

## Suggested fix
Reject any destination (and source) arrangement outside `{"8b","16b"}` before
computing `q`, mirroring validation already in `neon_arr_to_q_size`:
```rust
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("cnt: arrangement must be .8b or .16b, got .{}", arr_d));
}
```

## Correctness confirmed (properties that pass)
- `prop_cnt_matches_arm_reference` — golden encoding matches ARMv8 ARM reference
  (`cnt v0.8b,v0.8b` => `0x0e205800`, `cnt v0.16b,v0.16b` => `0x4e205800`).
- `prop_cnt_fields_isolated` — Rd in bits[4:0], Rn in bits[9:5], no leakage.
- `prop_cnt_q_and_fixed_bits` — Q (bit 30) is the only `.8b`/`.16b` delta.
- `golden_cnt_matches_llvm_mc`, `rejects_too_few_operands`.
