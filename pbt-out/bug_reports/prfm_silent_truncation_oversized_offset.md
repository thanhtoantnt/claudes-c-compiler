# Bug Report: `encode_prfm` silently truncates oversized immediate offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_prfm`
**Severity:** Medium

## Summary

`encode_prfm` accepts an arbitrary `i64` immediate and packs it into the 12-bit `imm12` field. The `as u32` cast happens *before* the range check, so any offset whose scaled value `imm/8` is `>= 2^32` wraps modulo `2^32` and may land in `0..=0xFFF`, passing the guard and emitting a word as if the offset were tiny. The field is only 12 bits; such offsets are unrepresentable and the ARM ARM requires the assembler to reject them.

## Root Cause

```rust
let imm12 = (imm / 8) as u32;     // i64 -> u32 narrowing FIRST
if imm12 > 0xFFF {                // range check AFTER the cast
    return Err(format!("prfm: offset too large: {}", imm));
}
```

## Reproduction

**Input:** `prfm pldl1keep,[x0,#34359738368]` (offset = 8·2^32, aligned, ≥ 0)

**Expected:** `Err` — offset too large

**Actual:** `Ok(Word(0xF9800400))` — encodes as `imm12=1` instead of rejecting

**Minimal failing input:** imm = 34359738368

## Impact

Gigantic aligned offsets encode as a *valid* small word with no diagnostic. For offsets where `imm/8 ≥ 2^32`, the wrapping produces completely wrong offset values. The assembler accepts invalid instructions and produces wrong machine code.

## Suggested Fix

Range-check the un-narrowed value before the cast:

```rust
let scaled = imm / 8;
if scaled > 0xFFF {
    return Err(format!("prfm: offset too large: {}", imm));
}
let imm12 = scaled as u32;
```

## Regression Property

Failing property: `prop_large_offset_not_silently_truncated`

```rust
prop_assert!(encode_prfm(&[Operand::Reg("x0".into()), mem_offset(xreg(1), 34359738368)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/120