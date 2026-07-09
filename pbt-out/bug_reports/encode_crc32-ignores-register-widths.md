# Bug Report: `encode_crc32` ignores register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_crc32`
**Severity:** Medium

## Summary

`encode_crc32` accepts `CRC32C` (`Wd`, `Wn`, `Wm`) but silently ignores their width constraints. ARMv8-A defines:
- **CRC32W** / **CRC32CW**: all three must be W registers
- **CRC32CX** / **CRC32X**: `Rd` must be X, `Rn/Rm` must be W

The encoder treats all as 32-bit, accepting invalid combinations and emitting UNALLOCATED encodings.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;
let (rm, _) = get_reg(operands, 2)?;
// No width validation
```

## Reproduction

**Input:** `crc32cx x0, w1, w2`

**Expected:** `Err` — CRC32X requires Xd, Wn, Wm

**Actual:** `Ok(Word(...))` — accepted, all treated as W

**Other failing inputs:** `crc32c w0, w1, w2` (reject), `crc32x w0, w1, w2` (reject), `crc32cx x0, x0, x2` (reject)

## Impact

Invalid width combinations accepted silently. CRC32C/X require specific width layouts that this encoder never enforces.

## Suggested Fix

Add width-specific validation:

```rust
let is_c = mnemonic.contains("crc32c");
let (rd, rd_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;

match (is_c, rd_64, rn_64, rm_64) {
    (false, true, true, true) => {},                                            // CRC32CW: Wd, Wn, Wm ✓
    (true, false, false, false) => {},                                          // CRC32X: Xd, Wn, Wm ✓
    _ => return Err("CRC32C/X requires: Wd, Wn, Wm (CRC32CW); Xd, Wn, Wm (CRC32X)").into()),
}
```

## Regression Property

Failing property: `crc32_rejects_width_violations`

```rust
prop_assert!(encode_crc32("crc32cx", &[xreg(0), wreg(1), wreg(2)]).is_err());
prop_assert!(encode_crc32("crc32x", &[wreg(0), wreg(1), wreg(2)]).is_err());
prop_assert!(encode_crc32("crc32c", &[xreg(0), wreg(1), wreg(2)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/142