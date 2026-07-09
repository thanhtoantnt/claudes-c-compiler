# Bug Report: `encode_rev32` (scalar) ignores register width, always emits 64-bit

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_rev32`
**Severity:** High

## Summary

Scalar branch of `encode_rev32` hardcodes `sf=1` and `opc=000010`, encoding `REV32 Wd, Wn` as `REV32 Xd, Xn`. Comment "REV32 is 64-bit only" is factually wrong. REV32 is defined for both widths.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
let (rn, _) = get_reg(operands, 1)?;
let word = ((1u32 << 31) | (1 << 30) | (0b011010110 << 21))  // sf=1 hardcoded
        | (0b000010 << 10) | (rn << 5) | rd;  // opc=000010 hardcoded (64-bit form)
```

## Reproduction

**Input:** `rev32 w0, w0`

**Expected:** `Ok(Word(0x5AC00C00))` — 32-bit form (sf=0, opc=000011)

**Actual:** `Ok(Word(0xDAC00800))` — 64-bit form (sf=1, opc=000010)

**Minimal failing input:** is_64 = false, rd = 0, rn = 0

## Impact

`REV32 Wd, Wn` silently widened to 64-bit operation. No error, disassembler reads back as `rev32 Xd, Xn`. Silent miscompilation.

## Suggested Fix

Derive `sf` and `opc` from register width, mirroring `encode_rev`:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
if is_64 != rn_64 {
    return Err("REV32 requires same-width registers".into());
}
let sf = sf_bit(is_64);
let opc: u32 = if is_64 { 0b000010 } else { 0b000011 };  // REV32 mirrors REV
let word = (sf << 31) | (1 << 30) | (0b011010110 << 21) | (opc << 10) | (rn << 5) | rd;
```

## Regression Property

Failing property: `prop_scalar_matches_arm_reference`

```rust
prop_assert_eq!(encode_rev32(&[wreg(0), wreg(0)]), Ok(EncodeResult::Word(0x5AC00C00)));  // W form
prop_assert_eq!(encode_rev32(&[xreg(0), xreg(0)]), Ok(EncodeResult::Word(0xDAC00800)));  // X form
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/153