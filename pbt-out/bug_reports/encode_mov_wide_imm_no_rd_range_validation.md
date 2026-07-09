# `encode_mov_wide_imm` does not validate the `rd` register field (`[4:0]`)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` →
`pub(crate) fn encode_mov_wide_imm(rd: u32, is_64: bool, imm: u64)`
**Witness property:** `mov_wide_rejects_rd_out_of_range`
(`#[ignore]`d, expected to fail)
**File:** `src/backend/arm/assembler/encoder/data_processing_mov_wide_imm_pbt.rs`
**Reproduce:** `cargo test --lib -- --ignored mov_wide_rejects_rd_out_of_range`

## Summary

`rd` is OR'd directly into bits `[4:0]` of every emitted word with no check that
it fits the 5-bit `Rd` field:

```rust
let word = (sf << 31) | (0b10100101 << 23) | (hw << 21) | (chunk << 5) | rd;
```

`Rd` is a 5-bit field in every AArch64 encoding, so `rd > 31` has no valid
representation and must be rejected. Instead the excess bits spill into the
`imm16` field, producing a silently corrupted word: `rd = 32` sets bit 5, turning
`imm16` from `0x1234` into `0x1235`.

## Falsifiable / minimal failing input

`rd = 32, is_64 = false, imm = 0x1234`.

- **property:** `mov_wide_rejects_rd_out_of_range`
- **actual:** `Ok(EncodeResult::Word(...))` with `imm16` field read-back = `0x1235`
  (bit 5 contaminated by `rd = 32`).
- **expected:** `Err` (register number out of range `[0, 31]`).

## Impact

A caller passing an unchecked register number corrupts the adjacent `imm16`
field — a silent miscompilation. Low practical likelihood (register numbers
usually come from the front-end), but the encoding is wrong with no diagnostic.

## Suggested fix

```rust
if rd > 31 {
    return Err(format!("register number {} out of range [0, 31]", rd));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/281
