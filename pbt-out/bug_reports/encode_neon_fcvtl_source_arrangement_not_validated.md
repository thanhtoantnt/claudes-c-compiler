# Bug Report: `encode_neon_fcvtl` does not validate the source register arrangement, and wrongly accepts a `.2s` destination

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_fcvtl`
**Severity:** Low

## Summary

`encode_neon_fcvtl` validates the **destination** register arrangement (rejecting
anything other than `.4s`/`.2s`/`.2d`) but completely ignores the **source**
register's arrangement: it is read and immediately discarded. The source type of
FCVTL/FCVTL2 is fully determined by the destination type (half→single: source
`.4h` (FCVTL) / `.8h` (FCVTL2); single→double: source `.2s` (FCVTL) / `.4s`
(FCVTL2)), and it must also match the `is_high` qualifier. A strict assembler
such as GNU `as` or `llvm-mc` requires the written source arrangement to match
the implied type and rejects mismatches as an operand-size mismatch. This encoder
accepts **any** source arrangement — including a bare register with no
arrangement at all — and silently produces the encoding implied by the
destination.

Additionally, the encoder accepts `.2s` as a **destination**. Per the ARMv8 ARM
there is no `.2s` FCVTL destination form (FCVTL widens a full source register, so
the only valid destinations are `.4s` and `.2d`). The encoder maps `.2s` to
`size=0` and emits the same word as a `.4s` destination, suppressing a
diagnostic. (`.2s` IS a valid *source* for the single→double (`.2d`) case — the
defect is specifically that it is accepted as a *destination*.)

The encoding bits themselves are correct for the (validated) destination; the
defects are the missing source-consistency check and the over-accepting
destination match arm, both of which suppress diagnostics for typos/mismatches in
the operands.

## Root Cause

```rust
pub(crate) fn encode_neon_fcvtl(operands: &[Operand], is_high: bool) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;        // <-- source arrangement discarded
    let sz = match arr_d.as_str() { "4s" | "2s" => 0u32, "2d" => 1,   // <-- ".2s" wrongly accepted
        _ => return Err(format!("fcvtl: unsupported dest: {}", arr_d)), };
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (0b01110 << 24) | (sz << 22) | (0b10000 << 17)
        | (0b10111 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_neon_reg(operands, 1)` returns `(rn, arr_n)` but `arr_n` is bound to `_` and
dropped. No comparison is ever made between the written source arrangement and
the arrangement implied by `arr_d` / `is_high`. The `"2s"` arm of the destination
`match` should be removed (or rejected) since it is not an architecturally valid
FCVTL destination.

## Reproduction

Witness tests live in
`src/backend/arm/assembler/encoder/neon_fcvtl_pbt.rs`, marked `#[ignore]` so the
default `cargo test` stays green. Run them explicitly:

```
cargo test --lib neon_fcvtl -- --ignored
```

Minimal failing inputs (both return `Ok(Word(0x0E217800))` instead of `Err`):

- `prop_fcvtl_rejects_invalid_source_arrangement` — `rd=0, rn=0`:
  `fcvtl v0.4s, v0.8b` → source `.8b` accepted; should be rejected (valid source
  for `.4s`/`is_high=false` is `.4h` only). Also accepts a bare source register.
- `prop_fcvtl_rejects_2s_destination` — `rd=0, rn=0, is_high=false`:
  `fcvtl v0.2s, v0.2h` → destination `.2s` accepted; should be rejected (no `.2s`
  FCVTL destination exists).

The passing `prop_fcvtl_rejects_invalid_dest_arrangements` confirms that other
invalid destinations (`8b`, `16b`, `4h`, `8h`, `1d`, `1q`) and bare destination
registers ARE correctly rejected.

## Suggested Fix

1. Read the source arrangement and validate it against the destination type and
   `is_high`, mirroring the FCVTN source check:
   - dest `.4s`, `is_high=false` → source must be `.4h`
   - dest `.4s`, `is_high=true`  → source must be `.8h`
   - dest `.2d`, `is_high=false` → source must be `.2s`
   - dest `.2d`, `is_high=true`  → source must be `.4s`
   - reject a bare source register.
2. Remove `"2s"` from the destination match arm (only `.4s` and `.2d` are valid
   FCVTL destinations).

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/238
