# `encode_neon_scalar_qshrn` does not range-check `u_bit`

- **Target:** `src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_scalar_qshrn`
- **Tests:** `src/backend/arm/assembler/encoder/neon_scalar_qshrn_pbt.rs`

## Minimal input
`SQSHRN B0, H0, #1` ⇒ operands `[Reg("b0"), Reg("h0"), Imm(1)]`, `u_bit=2`,
`is_rounding=false`.

## Expected vs actual
Expected: `Err` — `u_bit` is a single-bit field (bit 29) with no wrapping
semantics in the ARM Arm.
Actual: `Ok(0x0F0F9400)` — the value `2` is OR-shifted into bit 29, which sets
bit 30 and destroys the fixed `01` scalar marker (bits 31-30 become `00`).

## Impact
Any caller passing a stale/out-of-range `u_bit` silently produces a word in a
different encoding class rather than an error. The encoder cannot detect its own
misuse; downstream assembly is corrupt and unflagged.

## Root cause
`u_bit` is combined as `| (u_bit << 29)` with no `u_bit <= 1` guard.

## Fix
```rust
if u_bit > 1 {
    return Err(format!("scalar qshrn: u_bit {} out of range (must be 0 or 1)", u_bit));
}
```

## Witness
`#[ignore]`d test `prop_rejects_out_of_range_u_bit` fails today (run with
`cargo test --lib neon_scalar_qshrn_pbt -- --ignored prop_rejects_out_of_range_u_bit`).
The default `cargo test` run stays green.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/315
