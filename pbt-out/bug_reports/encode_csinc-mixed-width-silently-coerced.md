# Bug Report: `encode_csinc` silently accepts mixed register widths

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csinc`
**Severity:** High

## Summary

`encode_csinc` derives `sf` (width) bit from **only destination `Rd`**, discarding widths of `Rn` and `Rm`. Mixed-width combinations like `csinc w0, x1, x2, eq` silently coerced to `Rd`'s width. ARM ARM defines CSINC with single `sf` field applying to whole instruction — all registers must be same width.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;   // width from Rd ONLY
let (rn, _) = get_reg(operands, 1)?;      // Rn width DISCARDED
let (rm, _) = get_reg(operands, 2)?;      // Rm width DISCARDED
let sf = sf_bit(is_64);                   // = Rd's width
```

## Reproduction

**Input:** `csinc w0, w0, x0, eq`

**Expected:** `Err` — csinc: all registers must have the same width (X or W)

**Actual:** `Ok(Word(0x1A800400))` — same as `csinc w0, w0, w0, eq` (X0 silently reinterpreted as W0)

**Minimal failing input:** rd=0, rn=0, rm=0, rd_is64=false, rn_is64=false, rm_is64=true

## Impact

Silent mis-assembly: 64-bit source register reinterpreted as 32-bit. `csinc w0,w0,x0,eq` and `csinc w0,w0,w0,eq` encode identically. Same defect in `encode_csel`, `encode_csinv`, `encode_csneg`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if rd_is_64 != rn_is_64 || rn_is_64 != rm_is_64 {
    return Err("csinc: all registers must have the same width (X or W)".to_string());
}
```

## Regression Property

Failing property: `prop_rejects_mixed_width_registers`

```rust
prop_assert!(encode_csinc(&[wreg(0), wreg(0), xreg(0), cond("eq")]).is_err());
prop_assert!(encode_csinc(&[xreg(0), xreg(0), wreg(0), cond("eq")]).is_err());
```

## PBT Results (module `prop_encode_csinc_tests`)

| Property | Result |
|---|---|
| `prop_rejects_mixed_width_registers` | **FAIL** |
| `prop_rejects_zr_width_mismatch` | **FAIL** |
| `prop_uniform_widths_always_succeed` | PASS |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/168