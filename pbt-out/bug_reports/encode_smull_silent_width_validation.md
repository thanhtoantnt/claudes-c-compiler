# Bug Report: `encode_smull` silently accepts invalid operand widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smull`
**Severity:** High

## Summary

`encode_smull` discards width flags for all three operands. SMULL requires `Xd` (64-bit), `Wn`/`Wm` (32-bit). Invalid widths silently assembled, hiding operand-size mistakes.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;  // is_64 discarded
let (rn, _) = get_reg(operands, 1)?;  // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;  // is_64 discarded
let word = (1u32 << 31) | ...;        // sf hardwired to 1
```

## Reproduction

**Input:** `smull w0, w1, w2`

**Expected:** `Err` — SMULL requires Xd, Wn, Wm

**Actual:** `Ok(Word(...))` — W-register names accepted and encoded as 64-bit

**Other failing input:** `smull x0, x1, x2` (64-bit sources wrong)

## Impact

Wrong widths accepted, silently producing SMADDL 64-bit form with wrong register interpretations.

## Suggested Fix

Check all three `is_64` flags:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if !rd_is_64 || rn_is_64 || rm_is_64 {
    return Err("SMULL requires Xd, Wn, Wm".into());
}
```

## Regression Property

Failing property: `smull_rejects_wrong_width_destination`

```rust
prop_assert!(encode_smull(&[wreg(0), wreg(1), wreg(2)]).is_err());  // W destination
prop_assert!(encode_smull(&[xreg(0), xreg(1), xreg(2)]).is_err());  // X sources
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/97