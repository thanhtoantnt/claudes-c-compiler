# Bug Report: `encode_rev16` accepts mismatched Rd/Rn register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rev16`
**Severity:** Medium

## Summary

`encode_rev16` derives `sf` from destination `Rd` only, discarding source `Rn` width. ARMv8-A requires both registers to share same width via single `sf` field. Mixed-width forms like `rev16 x0, w1` accepted without diagnostic.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `rev16 w0, x1`

**Expected:** `Err` — REV16: Rd and Rn must have the same width

**Actual:** `Ok(Word(0x5AC00400))` — Rd=w0 (sf=0), x1 number reused as if w1

**Minimal failing input:** rd=0, rn=0, rd_is_64=false, rn_is_64=true

## Impact

UNPREDICTABLE/UNALLOCATED encodings emitted without diagnostic. Same class as `encode_rev`, `encode_rev32`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
if rd_is_64 != rn_is_64 {
    return Err("REV16: Rd and Rn must have the same width".into());
}
```

## Regression Property

Failing property: `prop_rejects_mismatched_widths`

```rust
prop_assert!(encode_rev16(&[xreg(0), wreg(1)]).is_err());  // mismatch
prop_assert!(encode_rev16(&[wreg(0), xreg(1)]).is_err());  // mismatch
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/151