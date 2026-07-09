# `encode_neon_scalar_qshrn` silently truncates out-of-range shift immediates

- **Target:** `src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_scalar_qshrn`
- **Tests:** `src/backend/arm/assembler/encoder/neon_scalar_qshrn_pbt.rs`

## Minimal input
`SQSHRN B0, H0, #(2^32 + 5)` ⇒ operands
`[Reg("b0"), Reg("h0"), Imm(4294967301)]`, `u_bit=0`, `is_rounding=false`.

## Expected vs actual
Expected: `Err` — the shift is far outside the valid `[1, dest_bits]` window.
Actual: `Ok(...)` — accepted, because `4294967301i64 as u32 == 5`, which falls in
range. A negative immediate such as `-4294967291` (= `-2^32 + 5`) likewise wraps
to `5` and is accepted.

## Impact
Malformed/garbage shift immediates are encoded as if valid instead of being
rejected. The range check `shift == 0 || shift > element_bits` runs only after
the `as u32` narrowing, so it never sees the true (truncated) magnitude. The
generated instruction is semantically wrong and unflagged.

## Root cause
`let shift = get_imm(operands, 2)? as u32;` narrows `i64 → u32` before the range
check, discarding high bits and reinterpreting negatives.

## Fix
Validate the `i64` before narrowing:
```rust
let shift_i = get_imm(operands, 2)?;
if shift_i <= 0 || shift_i as u64 > element_bits as u64 {
    return Err(format!("scalar qshrn: shift {} out of range", shift_i));
}
let shift = shift_i as u32;
```

## Witness
`#[ignore]`d test `prop_rejects_truncating_shift` fails today (run with
`cargo test --lib neon_scalar_qshrn_pbt -- --ignored prop_rejects_truncating_shift`).
The default `cargo test` run stays green.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/314
