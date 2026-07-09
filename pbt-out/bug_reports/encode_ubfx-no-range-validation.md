# Bug Report: `encode_ubfx` does not validate `lsb`/`width` immediates

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_ubfx`
**Severity:** High

## Summary

`encode_ubfx` accepts the `lsb` and `width` immediates via `get_imm(...)` as u32 with **no range validation**, then ORs them into the 32-bit encoding word. An architecturally out-of-range immediate is therefore silently truncated instead of being rejected with `Err`.

## Root Cause

```rust
let lsb = get_imm(operands, 2)? as u32;   // no range check
let width = get_imm(operands, 3)? as u32; // no range check
let immr = lsb + width - 1;
let word = (sf << 31) | ... | (immr << 16) | (imms << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

## Reproduction

**Input:** `ubfx w0, w1, #64, #1`

**Expected:** `Err` — for 32-bit register `lsb = 64` is invalid (must be ≤31)

**Actual:** `Ok(Word(0x5340820))` — out-of-range `lsb` overflows into adjacent opcode fields

**Minimal failing input:** is_64 = false, bad_lsb = 64, bad_width = 65

## Impact

Invalid assembler input silently produces a corrupted 32-bit opcode with no diagnostic. The user gets no diagnostic and downstream disassembly/simulation sees a wrong instruction. Same defect class as `encode_ubfm`, `encode_sbfm`, `encode_bfm`.

## Suggested Fix

Validate `lsb`/`width` against the register width:

```rust
let regsize = if is_64 { 64u32 } else { 32u32 };
if lsb >= regsize {
    return Err(format!("UBFX: lsb {} out of range [0, {}]", lsb, regsize));
}
if width == 0 || lsb + width > regsize {
    return Err(format!("UBFX: width {} out of range [1, {}]", width, regsize - lsb));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_lsb_width`

```rust
prop_assert!(encode_ubfx(&[wreg(0), wreg(1), imm(32), imm(1)]).is_err());  // lsb >= regsize
prop_assert!(encode_ubfx(&[xreg(0), xreg(1), imm(0), imm(65)]).is_err());  // width > regsize
prop_assert!(encode_ubfx(&[wreg(0), wreg(1), imm(0), imm(0)]).is_err());   // width=0
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/163