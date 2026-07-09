# Bug Report: `encode_bfxil` silently accepts out-of-range `lsb`/`width` (no range validation)

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_bfxil`
**Severity:** High

## Summary

`encode_bfxil` casts immediates with `as u32` and ORs directly into word without validation. `lsb >= 64` overflows `immr` into N bit [22]; `lsb + width - 1 >= 64` overflows `imms` into Rn field [9:5]. Negative immediates wrap via cast.

## Root Cause

```rust
let immr = lsb;
let imms = lsb + width - 1;
let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23)
         | (n << 22) | (immr << 16) | (imms << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `bfxil x0, x1, #100, #1`

**Expected:** `Err` — lsb 100 out of range (lsb must be < 64)

**Actual:** `Ok(Word(...))` — immr overflow into N bit and opcode bits

**Minimal failing inputs:** lsb=100 (overflows immr), lsb=63 width=2 (overflows imms), lsb=-1 (negative wraps)

## Impact

Silent mis-encoding: emits incorrect machine code for malformed BFXIL operands. Wrong instruction still parses as valid shape downstream — error invisible until runtime. Same pattern in `encode_ubfx`, `encode_sbfx`, raw BFM-family encoders.

## Suggested Fix

Validate ranges before building word:

```rust
let regsize = if is_64 { 64u32 } else { 32 };
if lsb >= regsize || width == 0 || lsb + width > regsize {
    return Err(format!("BFXIL: lsb/width out of range (lsb={}, width={}, regsize={})",
                       lsb, width, regsize));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_operands`

```rust
prop_assert!(encode_bfxil(&[xreg(0), xreg(1), imm(100), imm(1)]).is_err());  // lsb overflow
prop_assert!(encode_bfxil(&[xreg(0), xreg(1), imm(63), imm(2)]).is_err());   // imms overflow
prop_assert!(encode_bfxil(&[xreg(0), xreg(1), imm(-1), imm(1)]).is_err());   // negative wraps
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/137