# Bug Report: `encode_neon_sshr` accepts out-of-range shifts, emits UNDEFINED words

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sshr`
**Severity:** Medium

## Summary

`encode_neon_sshr` performs no range validation. Shifts outside `[1, esize]` are accepted; `immh` can collapse to `0000` (UNALLOCATED) or re-encode as a different element size.

## Root Cause

```rust
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,  // no range check
    ...
};
```

## Reproduction

**Input:** `sshr v0.8b, v1.8b, #0`

**Expected:** `Err` — llvm-mc rejects with "immediate must be in range [1, 8]"

**Actual:** `Ok(Word(0x0F000420))` — immh=0000 UNALLOCATED

## Impact

Malformed shifts silently become wrong/UNDEFINED encodings.

## Suggested Fix

```rust
if !(1..=esize).contains(&shift) {
    return Err(format!("sshr: shift {} out of range [1, {}]", shift, esize));
}
```

## Regression Property

Failing property: `sshr_rejects_out_of_range_shift`

```rust
prop_assert!(encode_neon_sshr(&[vreg(0, "8b"), vreg(1, "8b"), Imm(0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/244
