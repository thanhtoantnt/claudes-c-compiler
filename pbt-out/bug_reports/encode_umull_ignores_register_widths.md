# Bug Report: `encode_umull` silently ignores register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umull`
**Severity:** Medium

## Summary

`UMULL <Xd>, <Wn>, <Wm>` decodes each operand's *number* but discards its *width* (the `is_64` half of `get_reg`'s return). It therefore accepts `umull w0, w1, w2` — which has **no valid AArch64 encoding** — and emits the word for `umull x0, w1, w2` instead.

Per the ARMv8 ARM, `UMULL <Xd>, <Wn>, <Wm>` is the alias of `UMADDL <Xd>, <Wn>, <Wm>, <XZR>` whose encoding fixes `sf = 1` (64-bit destination). The destination **must** be 64-bit (X) register and the sources **must** be 32-bit (W`) registers.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
// sf is hardcoded to 1 below regardless of operands' actual widths
let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
```

`get_reg` returns `(num, is_64)`; all three bindings use `_`, so width validation is never performed.

## Reproduction

**Input:** `umull w0, w1, w2`

**Expected:** `Err` — umull requires 64-bit destination and 32-bit sources

**Actual:** `Ok(EncodeResult::Word(0x9BA07C02))` — encodes as `umull x0, w1, w2`

**Minimal failing input:** rd = 0, rn = 0, rm = 0

## Impact

Malformed source lines assemble without any diagnostic. The problem hides in plain sight because the word encodings happen to match the same bit fields as valid forms.

## Suggested Fix

Validate widths against the `UMULL <Xd>, <Wn>, <Wm>` contract:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !rd64 {
    return Err("umull requires a 64-bit destination (Xd)".to_string());
}
if rn64 || rm64 {
    return Err("umull requires 32-bit sources (Wn/Wm)".to_string());
}
let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (ra << 10) | (rn << 5) | rd;
```

## Regression Property

Failing property: `umull_rejects_width_violations`

```rust
prop_assert!(encode_umull(&[wreg(0), wreg(1), wreg(2)]).is_err());  // W destination
prop_assert!(encode_umull(&[xreg(0), xreg(1), xreg(2)]).is_err());  // X sources
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/201