# Bug Report: `encode_bic` silently truncates out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_bic` (shifted-register form, scalar)
**Severity:** Medium

## Summary

`encode_bic`'s shifted-register branch writes the shift amount into the 6-bit `imm6` field with a bare `& 0x3F` mask and **no range validation**:

```rust
let word = (sf << 31) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

Per the ARMv8 ARM (§C4.1.4, *Logical (shifted register)*), the `imm6` field is only 6 bits, so a shift amount **> 63 is unrepresentable** and the mnemonic must be rejected. Instead the encoder masks the value and emits a valid-looking — but semantically wrong — instruction.

## Root Cause

The code masks the shift amount with `& 0x3F` without validating that the value fits in the 6-bit `imm6` field. For 64-bit (X) registers the valid range is `0..=63`; for 32-bit (W) registers the range is `0..=31` (bit 5 of `imm6` must be 0). Out-of-range values silently wrap, producing reserved/UNDEFINED encodings.

## Reproduction

**Input:** `bic x0, x1, x2, lsl #64`

**Expected:** `Err` — shift amount out of range (0..=63 for X-registers)

**Actual:** `Ok(Word(_))` — silently encoded as `lsl #0`

**Minimal failing input:** `bic x0, x1, x2, lsl #64`

| Mnemonic                     | Expected | Actual                                   |
|------------------------------|----------|------------------------------------------|
| `bic Xd,Xn,Xm, lsl #64`      | `Err`    | `Ok` → encoded as `lsl #0`               |
| `bic Xd,Xn,Xm, lsl #65`      | `Err`    | `Ok` → encoded as `lsl #1`               |
| `bic Wd,Wn,Wm, lsl #32`      | `Err`    | `Ok` → reserved encoding (imm6 bit 5 = 1)|

## Impact

Silent mis-compilation: invalid assembly instructions are accepted and produce valid-but-wrong encodings. Users write instructions that differ from what the assembler emits, with no diagnostic. This can lead to incorrect program behavior that is difficult to debug.

## Suggested Fix

Validate the shift amount against the operand width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "bic shift amount {} out of range [0, {}]", shift_amount, max_shift
    ));
}
```

(For `ror` on 64-bit registers the architecturally-valid range is `1..=63`; `0` is CONSTRAINED UNPREDICTABLE — at minimum reject `> 63` as above.)

## Regression Property

Failing property: `bic_rejects_oversized_shift`

```rust
prop_assert!(encode_bic(&[xreg(rd), xreg(rn), xreg(rm)], "lsl", 64).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/9