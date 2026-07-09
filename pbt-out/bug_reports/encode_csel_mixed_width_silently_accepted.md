# Bug Report: `encode_csel` silently accepts mismatched register widths

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csel`
**Severity:** Medium

## Summary

`encode_csel` derives `sf` (register-width) bit **solely from destination `Rd`**, discarding width of `Rn` and `Rm`. Mixed-width operand sets silently accepted and emitted as either 64-bit or 32-bit instruction depending only on `Rd`.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // ← width discarded
let (rm, _) = get_reg(operands, 2)?;   // ← width discarded
let sf = sf_bit(is_64);                 // ← derived ONLY from Rd
```

ARM ARM CSEL requires `Rd`, `Rn`, `Rm` all same width — all X (sf=1) or all W (sf=0).

## Reproduction

**Input:** `csel x0, w1, w2, eq`

**Expected:** `Err` — Rd, Rn and Rm must all be the same register width

**Actual:** `Ok(Word(0x1A82_0020))` — 64-bit CSEL with X1,X2 (W1,W2 silently rewritten)

## Impact

Silent coercion produces instruction user never wrote. 32-bit W register as 64-bit source reads undefined high bits; 64-bit X truncated to W loses data. Every real AArch64 assembler treats this as error.

## Suggested Fix

Validate width consistency:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if rn_is_64 != is_64 || rm_is_64 != is_64 {
    return Err("csel: Rd, Rn and Rm must all be the same register width".to_string());
}
```

## Regression Property

Failing property: `prop_rejects_mismatched_register_widths`

```rust
prop_assert!(encode_csel(&[xreg(0), wreg(1), wreg(2), cond("eq")]).is_err());
prop_assert!(encode_csel(&[wreg(0), xreg(1), xreg(2), cond("eq")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/167