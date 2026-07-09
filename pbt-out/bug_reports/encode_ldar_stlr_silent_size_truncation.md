# Bug Report: `encode_ldar_stlr` silently corrupts out-of-range `forced_size`

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldar_stlr`
**Severity:** High

## Summary

The `size` field is computed as `forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 })` and placed into bits `[31:30]` via `size << 30` **without any range validation**.

The ARMv8-A Architecture Reference Manual only allocates `size` encodings `0b00`–`0b11` for the LDAR/STLR instruction family (LDAR/STLR, LDARB/STLRB, LDARH/STLRH; size=10/11 are the 32/64-bit forms). Any `forced_size` value `>= 4` therefore has no valid encoding and *should* be rejected with `Err`, but instead it silently shifts off the high bits and emits a corrupt instruction word.

## Root Cause

```rust
let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
...
let word = ((size << 30) | (0b001000 << 24) | (1 << 23) | (l << 22))
    | (0b11111 << 16) | (1 << 15) | (0b11111 << 10) | (rn << 5) | rt;
```

`size << 30` for `size >= 4` discards the bits above position 31:
- `forced_size = Some(4)`  → `4u32 << 30 == 0` → `size` field becomes `00` (LDARB) silently
- `forced_size = Some(5)`  → `5u32 << 30 == 0x40000000` → `size` field becomes `01` (LDARH) silently
- `forced_size = Some(255)`→ `size` field becomes `11` silently

No `Err` is ever produced for these unallocatable values.

## Reproduction

**Input:** `stlr x0, [x1]` with `forced_size = Some(4)`

**Expected:** `Err` — forced_size 4 is out of range (valid values: 0..=3)

**Actual:** `Ok(Word(0x8A020020))` — silently corrupts to a different size encoding

**Minimal failing input:** bad_size = 4, is_load = false

## Impact

Silent mis-compilation: invalid `forced_size` values produce corrupt instruction words that encode to completely different instruction types (e.g., LDAR silently becomes LDARB or LDARH). This breaks the semantic contract of the encoder and produces wrong machine code with no diagnostic.

## Suggested Fix

Validate `forced_size` before encoding:

```rust
if let Some(size) = forced_size {
    if size > 3 {
        return Err(format!("forced_size {} out of range (0..=3)", size));
    }
}
```

## Regression Property

Failing property: `prop_forced_size_out_of_range_rejected`

```rust
prop_assert!(encode_ldar_stlr(&[xreg(0), mem(xreg(1))], false, Some(4)).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/43