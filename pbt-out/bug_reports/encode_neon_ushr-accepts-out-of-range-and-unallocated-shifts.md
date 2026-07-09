# Bug Report: `encode_neon_ushr` silently accepts out-of-range / unallocated shifts

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ushr`
**Severity:** Medium

## Summary

`encode_neon_ushr` never validates the shift amount. Values outside `[1, esize]` are accepted; `immh` can become `0000` (UNALLOCATED).

## Root Cause

```rust
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,  // no range check
    ...
};
```

## Reproduction

**Input:** `ushr v0.8b, v1.8b, #0`

**Expected:** `Err` — immh would be 0000 UNALLOCATED

**Actual:** `Ok(word)` with immh=0

## Impact

Malformed `#shift` silently becomes wrong/UNDEFINED encoding.

## Suggested Fix

```rust
if shift == 0 || shift > max {
    return Err(format!("ushr: shift {} out of range [1, {}]", shift, max));
}
```

## Regression Property

Failing property: `prop_out_of_range_shifts_must_be_rejected`

```rust
prop_assert!(encode_neon_ushr(&[vreg(0,"8b"), vreg(1,"8b"), Imm(0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/248
