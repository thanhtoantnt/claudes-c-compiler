# Bug Report: `encode_smaddl` silently accepts a 32-bit destination register

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smaddl`
**Severity:** High

## Summary

`encode_smaddl` discards all `is_64` flags from `get_reg`. SMADDL is 64-bit only (`Xd, Wn, Wm, Xa`). Accepts 32-bit `W` destination, emitting architecturally UNDEF word.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded
let word = (1u32 << 31) | ...;         // sf hardwired to 1 (correct for 64-bit form)
```

## Reproduction

**Input:** `smaddl w0, x1, x2, x3`

**Expected:** `Err` — smaddl destination must be a 64-bit (X) register

**Actual:** `Ok(Word(0x9B200000))` — same word as `smaddl x0, x1, x2, x3`

**Minimal failing input:** rd=0, rn=0, rm=0, ra=0 (all W forms)

## Impact

Malformed source assembled into architecturally UNDEF word. Same defect in `encode_umaddl`.

## Suggested Fix

Enforce width contract `Xd, Wn, Wm, Xa`:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
let (ra, ra64) = get_reg(operands, 3)?;
if !rd64 { return Err("smaddl destination must be 64-bit (X)".into()); }
if !ra64 { return Err("smaddl accumulator must be 64-bit (X)".into()); }
if rn64 || rm64 { return Err("smaddl sources must be 32-bit (W)".into()); }
```

## Regression Property

Failing property: `smaddl_rejects_w_destination_register`

```rust
prop_assert!(encode_smaddl(&[wreg(0), wreg(1), wreg(2), xreg(3)]).is_err());  // W destination
prop_assert!(encode_smaddl(&[xreg(0), xreg(1), xreg(2), xreg(3)]).is_err());  // X sources
prop_assert!(encode_smaddl(&[xreg(0), wreg(1), wreg(2), wreg(3)]).is_err());  // W accumulator
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/96