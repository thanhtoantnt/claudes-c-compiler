# Bug Report: `encode_csinv` does not validate mixed register widths

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csinv`
**Severity:** High

## Summary

`encode_csinv` silently accepts mixed-width register operands (e.g. `csinv w0, w0, x0, eq`). The `sf` bit governs width of `Rd`, `Rn`, `Rm` collectively, so non-uniform width is UNALLOCATED and must be rejected.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;   // width taken ONLY from Rd
let (rn, _) = get_reg(operands, 1)?;        // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;        // is_64 discarded
```

Only `is_64` from operand 0 (`Rd`) is used; widths of `Rn` and `Rm` are discarded.

## Reproduction

**Input:** `csinv w0, w0, x0, eq`

**Expected:** `Err` — all register operands must have the same width

**Actual:** `Ok(Word(0x5A800000))` — 64-bit source silently re-encoded as 32-bit

**Minimal failing input:** rd=0, rn=0, rm=0, rd_is64=false, rn_is64=false, rm_is64=true

## Impact

Malformed instruction word emitted for any mixed-width CSINV. Reverse case (`csinv x0, x0, w0, eq`) also accepted. Downstream consumers receive well-formed but semantically wrong word.

## Suggested Fix

Validate width agreement:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if rn_is_64 != is_64 || rm_is_64 != is_64 {
    return Err("csinv: all register operands must have the same width".into());
}
```

## Regression Property

Failing property: `prop_rejects_mixed_width_operands`

```rust
prop_assert!(encode_csinv(&[wreg(0), wreg(0), xreg(0), cond("eq")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/164