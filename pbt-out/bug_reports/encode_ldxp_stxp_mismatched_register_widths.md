# Bug Report: `encode_ldxp_stxp` accepts mismatched pair register widths

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldxp_stxp`

## Summary

The LDXP/STXP pair operands must have matching widths. `encode_ldxp_stxp` derives the `sz` bit only from the first pair register (`Rt`) and does not verify that `Rt2` has the same W/X class. Mixed-width pairs are accepted and encoded as architecturally UNDEFINED instructions.

## Reproduction

Direct probe from the PBT campaign:

```text
ldxp w0, x1, [x2]
```

Actual result:

```text
Ok(Word(0x887F0440))
```

The encoder chooses 32-bit form from `w0` while still encoding `x1` as the second register.

## Impact

The assembler emits undefined pair-exclusive encodings instead of rejecting invalid mixed-width source operands.

## Suggested fix

Track the width class of both pair registers and reject mismatches:

```rust
let (rt, rt_is_64) = get_reg(operands, rt_index)?;
let (rt2, rt2_is_64) = get_reg(operands, rt2_index)?;
if rt_is_64 != rt2_is_64 {
    return Err("ldxp/stxp pair registers must have matching widths".to_string());
}
```

## Regression property

Failing property: `ldxp_stxp_mismatched_widths_rejected`

```rust
prop_assert!(encode_ldxp_stxp(&[wreg(0), xreg(1), xreg(2), xreg(3)], true).is_err());
```
