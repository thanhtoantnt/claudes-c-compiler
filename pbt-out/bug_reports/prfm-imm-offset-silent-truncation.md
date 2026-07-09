# Bug Report: `encode_prfm` immediate form silently truncates out-of-range offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_prfm` (immediate form)
**Severity:** High

## Summary

PRFM immediate form computes `(imm / 8) as u32` **before** the `> 0xFFF` range check. When `imm/8 >= 2^32`, truncating cast wraps to small `u32`, range check passes, and encoder silently mis-assembles with tiny offset.

## Root Cause

```rust
let imm12 = (imm / 8) as u32;           // wraps for imm/8 >= 2^32
if imm12 > 0xFFF { return Err(...); }   // check sees wrapped value
```

## Reproduction

**Input:** `prfm pldl1keep, [x0, #34359738368]` (imm/8 = 2^32)

**Expected:** `Err` — PRFM offset out of range

**Actual:** `Ok(Word(0xF9800000))` — identical to `[x0, #0]` (offset silently dropped)

**Minimal failing input:** scaled = 4294967296 (imm = 34359738368)

## Impact

Silent mis-assembly: huge offsets produce wrong instructions. No diagnostic.

## Suggested Fix

Check on `i64` before cast:

```rust
let scaled = imm / 8;
if scaled < 0 || scaled > 0xFFF {
    return Err(format!("prfm: offset out of range: {}", imm));
}
let imm12 = scaled as u32;
```

## Regression Property

Failing property: `prop_prfm_large_offset_not_silently_truncated`

```rust
prop_assert!(encode_prfm(&[Operand::Reg("x0".into()), mem_offset(xreg(1), 34359738368)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/208