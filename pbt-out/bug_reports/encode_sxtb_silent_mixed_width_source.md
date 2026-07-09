# Bug Report: `encode_sxtb` silently accepts mixed-width source register

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtb`
**Severity:** Medium

## Summary

`encode_sxtb` derives `sf` from destination `Rd` only, discarding source `Rn` width. ARMv8-A requires source/destination registers to share width. Mixed-width forms like `sxtb x0, w0` accepted without diagnostic.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
```

## Reproduction

**Input:** `sxtb x0, w0`

**Expected:** `err("SXTB requires same-width registers")`

**Actual:** `Ok(EncodeResult::Word(...))` — encodes as SXTB X0, X0

**Minimal failing input:** rd="x0", rn="w0"

## Impact

Mixed-width forms accepted silently. Same defect class as `encode_uxth`, `encode_sxth`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
if is_64 != rn_64 {
    return Err("SXTB requires same-width registers".into());
}
```

## Regression Property

Failing property: `sxtb_rejects_mixed_width`

```rust
prop_assert!(encode_sxtb(&[xreg(0), wreg(0)]).is_err());
prop_assert!(encode_sxtb(&[wreg(0), xreg(0)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/164