# Bug Report: `encode_umaddl` performs no register-width validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umaddl`
**Severity:** Medium

## Summary

`UMADDL Xd, Wn, Wm, Xa` is an unsigned multiply-add long instruction (`Xa + Wn * Wm`, widened to 64 bits). Per the ARMv8 ARM it is **only** in the 64-bit (`sf=1`) form, with fixed operand-width contract: destination `Xd` and accumulator `Xa` MUST be 64-bit (`X`) registers, while the two multiplier sources `Wn`/`Wm` MUST be 32-bit (`W`) registers.

The encoder calls `get_reg` for all four operands but **discards the `is_64` flag** on every one, performing zero width checks. Mixed W/X operands are silently re-typed and emitted as a word indistinguishable from a correctly-typed `UMADDL`.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded — Rd must be X
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded — Rn must be W
let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded — Rm must be W
let (ra, _) = get_reg(operands, 3)?;   // is_64 discarded — Ra must be X
// sf is hardwired to 1 below regardless of operands' actual widths
let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
    | (ra << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `umaddl w0, w1, w2, x3`

**Expected:** `Err` — source operands must be 32-bit (W)

**Actual:** `Ok(Word(0x9BA00000))` — emits same as `umaddl x0, x1, x2, x3`

**Minimal failing input:** rd = 0, rn = 0, rm = 0, ra = 0 (all W sources)

## Impact

Malformed source lines assemble without any diagnostic into architecturally UNDEFINED encodings. A wrong-width operand lands in the same bit field as its valid counterpart.

## Suggested Fix

Enforce the fixed-width contract before emitting:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
if !rd64 {
    return Err("umaddl destination must be a 64-bit (X) register".to_string());
}
if ra64 {
    return Err("umaddl accumulator must be a 64-bit (X) register".to_string());
}
if rn64 || rm64 {
## Regression Property

Failing property: `umaddl_rejects_width_violations`

```rust
prop_assert!(encode_umaddl(&[wreg(0), wreg(1), wreg(2), xreg(3)]).is_err());  // W destination
prop_assert!(encode_umaddl(&[xreg(0), xreg(1), xreg(2), xreg(3)]).is_err());  // X sources
prop_assert!(encode_umaddl(&[xreg(0), wreg(1), wreg(2), wreg(3)]).is_err());  // W accumulator
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/104