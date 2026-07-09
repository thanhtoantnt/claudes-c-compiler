# Bug — `encode_neon_shll`: over-range shift silently encodes a DIFFERENT element width

**Target:** `src/backend/arm/assembler/encoder/neon.rs`, `pub(crate) fn encode_neon_shll` (lines ~1472–1483)
**Test:** `src/backend/arm/assembler/encoder/neon_shll_pbt.rs` — `prop_shll_rejects_over_range_shift` (failing, shrunk PBT property) + deterministic witness `over_range_shift_8_on_8b_silently_changes_size`

## Summary

The SHLL/SHLL2 (Advanced SIMD shift left *long*) encoder performs **no range
check** on the shift immediate. A shift outside the valid range `0..=esize-1`
(7 / 15 / 31 for the `.8b` / `.4h` / `.2s` source families) is accepted (`Ok`)
and the resulting `immh:immb = esize + shift` lands in the **next width
category**, so the emitted word decodes as a different source element size than
the arrangement the programmer requested (silent instruction-size corruption).
A real assembler rejects this; `encode_neon_shll` does not. Its sibling
`encode_neon_qshrn` does validate (`if shift > element_bits { return Err }`) —
this guard was simply omitted here.

## Minimal failing input (shrunk PBT witness)

`prop_shll_rejects_over_range_shift` shrunk to: `rd=0, rn=0, arr_n="8b", shift=9, u_bit=0, is_high=false`.

Equivalent assembly: `sshll v0.8h, v0.8b, #9`.

reproduce: `cargo test --lib neon_shll_pbt::prop_shll_rejects_over_range_shift`

## Expected vs actual

- **Expected:** `Err` — shift `9` exceeds the valid range `0..=7` for an 8-bit
  source (ARMv8 ARM "Advanced SIMD shift by amount", long group).
- **Actual:** `Ok(Word(0x0F11_A400))`. The encoded `immh = 0010` decodes as a
  **16-bit source** (`.4h`/`.8h` family), not the requested `.8b`.

PBT minimal failing input (proptest): `rd=0, rn=0, arr_n="8b", shift=9,
u_bit=0, is_high=false`.

## Impact

Corrupt instruction word emitted silently. The assembled program behaves as a
different instruction than the source text describes, with no diagnostic.
Reachable from any caller passing a parsed `Operand::Imm` that an upper layer
failed to clamp.

## Fix

Mirror `encode_neon_qshrn`'s guard, before computing `immhb`:

```rust
let esize = base_val; // 8 / 16 / 32
if shift > esize - 1 {           // valid SHLL shift is 0..=esize-1
    return Err(format!("sshll/ushll: shift {shift} out of range for {esize}-bit source"));
}
```

## Reproduce

```
cargo test --lib neon_shll_pbt::over_range_shift_8_on_8b_silently_changes_size
cargo test --lib neon_shll_pbt::prop_shll_rejects_over_range_shift
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/95
