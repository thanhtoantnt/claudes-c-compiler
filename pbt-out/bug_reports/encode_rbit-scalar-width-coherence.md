# Bug Report: `encode_rbit` scalar form has mismatched width handling

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rbit`
**Severity:** High

## Summary

`encode_rbit` (scalar form) derives instruction width from destination `Rd` only, discarding source `Rn` width. ARMv8-A requires same width for both registers. Mixed-width forms like `rbit x0, w1` accepted without diagnostic.

## Root Cause

```rust
let (rd, rd_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let word = ((rd_64 as u32) << 31) | (0b110101011 << 21)
         | (0b00010 << 16) | (rn << 5) | rd;
```

## Reproduction

**Input:** `rbit x0, w1`

**Expected:** `RBIT requires same-width registers (both X or both W)`

**Actual:** `Ok(Word(...))` — encodes as 64-bit RBIT using W1 number

**Minimal failing input:** rd="x0", rn="w0" (or reverse)

## Impact

Mixed-width forms accepted silently. Same defect class as `encode_cls`, `encode_clz`, `encode_rev16`, `encode_rev32`, `encode_rev`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, rd_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
if rd_64 != rn_64 {
    return Err("RBIT requires same-width registers".into());
}
```

## Regression Property

Failing property: `prop_rbit_rejects_mixed_width_operands`

```rust
prop_assert!(encode_rbit(&[xreg(0), wreg(1)]).is_err());
prop_assert!(encode_rbit(&[wreg(0), xreg(1)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/101