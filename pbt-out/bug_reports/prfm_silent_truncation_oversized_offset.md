# Bug — `encode_prfm` silently truncates oversized immediate offsets

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `pub(crate) fn encode_prfm` — `Operand::Mem` arm
**Differential oracle:** ARMv8-A ARM §C4.1.89 (imm12 = pimm/8, pimm ∈ [0, 32760])
**Severity:** Medium — gigantic aligned offsets encode as a *valid* small word
with no diagnostic instead of being rejected.

## Root cause

```rust
let imm12 = (imm / 8) as u32;     // i64 -> u32 narrowing FIRST
if imm12 > 0xFFF {                // range check AFTER the cast
    return Err(format!("prfm: offset too large: {}", imm));
}
```

The `as u32` cast happens *before* the range check, so any offset whose scaled
value `imm/8` is `>= 2^32` wraps modulo `2^32` and may land in `0..=0xFFF`,
passing the guard and emitting a word as if the offset were tiny. The field is
only 12 bits; such offsets are unrepresentable and the ARM ARM requires the
assembler to reject them.

## Minimal failing input (Property 5)

`scaled = 4294967296` (`= 2^32`), `imm = 34359738368` (`8·2^32`, aligned, ≥ 0).
Crate returns `Ok(Word(0xF9800400))` — i.e. encodes `prfm pldl1keep,[x0,#8]`
(imm12=1) — instead of `Err`. `(34359738368 / 8) as u32 == 0x1_0000_0000 as u32 == 0`.

```
cargo test --lib prop_encode_prfm_tests::prop_prfm_large_offset_not_silently_truncated
```

## Fix

Range-check the un-narrowed value before the cast:

```rust
let scaled = imm / 8;
if scaled > 0xFFF {
    return Err(format!("prfm: offset too large: {}", imm));
}
let imm12 = scaled as u32;
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/120
