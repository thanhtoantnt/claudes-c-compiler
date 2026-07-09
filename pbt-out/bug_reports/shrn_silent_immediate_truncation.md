# Bug Report: `encode_neon_shrn` silently truncates out-of-range shift immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_shrn`
**Severity:** Medium

## Summary

`encode_neon_shrn` casts the shift `i64 -> u32` before range check. Values whose low 32 bits land in `[1, half_bits]` are silently accepted as the truncated shift.

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;  // truncation first
if shift == 0 || shift > half_bits { return Err(...); }  // checks truncated value
```

## Reproduction

**Input:** `shrn v0.8b, v1.8h, #0x100000001`

**Expected:** `Err` — out of range

**Actual:** `Ok(Word(0x0F0F8420))` — identical to `#1`

## Impact

Huge immediates silently wrap to small legal shifts — wrong encoding, no diagnostic.

## Suggested Fix

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i <= 0 || shift_i as u64 > half_bits as u64 {
    return Err(...);
}
```

## Regression Property

Failing property: `prop_shrn_truncates_huge_immediate`

```rust
prop_assert!(encode_neon_shrn(&[vreg(0,"8b"), vreg(1,"8h"), Imm(0x1_0000_0001)], ...).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/254
