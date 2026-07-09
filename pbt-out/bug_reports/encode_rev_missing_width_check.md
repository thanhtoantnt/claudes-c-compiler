# Bug Report: `encode_rev` silently accepts mismatched Rd/Rn register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rev`
**Severity:** High

## Summary

`encode_rev` derives `sf` solely from destination `Rd`, discarding source `Rn` width. ARMv8-A REV encoding has single `sf` field, so `Rd` and `Rn` must share width. Mixed-width forms accepted and encoded as UNPREDICTABLE/UNALLOCATED.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `rev w0, x0`

**Expected:** `Err` — REV requires Rd and Rn of same width

**Actual:** `Ok(Word(0x5AC00800))` — x0 encoded as w0 (width info lost)

**Minimal failing input:** rd=0, rn=0, rd_is_64=false (rn_is_64=true)

## Impact

UNPREDICTABLE/UNALLOCATED encodings emitted without diagnostic. Source register width info silently dropped.

## Suggested Fix

Validate width coherence:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
if is_64 != rn_is_64 {
    return Err("REV requires Rd and Rn of the same width".into());
}
```

## Regression Property

Failing property: `prop_rejects_mismatched_widths`

```rust
prop_assert!(encode_rev(&[wreg(0), xreg(0)]).is_err());  // W dest, X src
prop_assert!(encode_rev(&[xreg(0), wreg(0)]).is_err());  // X dest, W src
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/154