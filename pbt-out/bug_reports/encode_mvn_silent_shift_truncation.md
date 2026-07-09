# Bug Report: `encode_mvn` silently truncates out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mvn`
**Severity:** Medium

## Summary

`MVN Xd, Xm [, <shift> #amount]` masks the shift amount with `& 0x3F` before placing it in the imm6 field and never validates the range. A shift amount greater than 63 overflows the 6-bit imm6 field and is **silently wrapped** rather than rejected. Reference assemblers reject the same input.

## Root Cause

```rust
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
```

The `& 0x3F` mask silently truncates; there is no bounds check returning `Err`.

## Reproduction

**Input:** `mvn x0, x0, lsl #64`

**Expected:** `Err` — immediate value out of range

**Actual:** `Ok(Word(_))` — encoded identically to `mvn x0, x0` (no shift)

**Minimal failing input:** rd = 0, rm = 0, amount = 64

## Impact

Invalid assembly instructions are accepted and produce wrong encodings. The programmer's intended shift amount is corrupted (e.g., `lsl #100` becomes `lsl #36`). Same pattern affects `encode_orn`, `encode_eon`, `encode_bics`, `encode_bic`, `encode_logical`, `encode_neg`, `encode_negs`.

## Suggested Fix

Validate the shift amount against the register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-bit register", shift_amount,
        if is_64 { 64 } else { 32 }
    ));
}
```

For 32-bit (W) registers, imm6 values 32..=63 are architecturally UNPREDICTABLE; GAS rejects them too.

## Regression Property

Failing property: `out_of_range_shift_amount_is_rejected`

```rust
prop_assert!(encode_mvn(&[xreg(0), xreg(0)], "lsl", 64).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/72