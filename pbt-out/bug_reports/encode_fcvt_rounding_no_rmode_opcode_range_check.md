# Bug Report: `encode_fcvt_rounding` performs no range validation on `rmode` / `opcode`

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fcvt_rounding`
**Severity:** Medium

## Summary

`encode_fcvt_rounding` ORs `rmode` and `opcode` into fixed-width instruction fields with **no range check**. Per ARMv8-A, `rmode` is a **2-bit** field at bits [20:19] (legal values 0–3) and `opcode` is a **3-bit** field at bits [18:16] (legal values 0–7). Values outside these ranges are not masked or rejected; they spill into neighbouring fields, corrupting `ftype` and `rmode`.

## Root Cause

```rust
let word = ((sf << 31) | (0b11110 << 24) | (ftype << 22)
    | (1 << 21) | (rmode << 19) | (opcode << 16)) | (rn << 5) | rd;
```

No validation that `rmode <= 0b11` or `opcode <= 0b111` before encoding.

## Reproduction

**Input:** `encode_fcvt_rounding([w0,s0], 8, 0)` (rmode=8 exceeds 2-bit field)

**Expected:** `Err` — rmode exceeds 2-bit field [20:19]

**Actual:** `Ok(Word(0x1E600000))` — silently corrupts ftype (single→double)

**Minimal failing input:** rmode = 4 or opcode = 8

## Impact

`rmode >= 4` corrupts `ftype`, silently re-encoding single-precision as double. `opcode >= 8` corrupts `rmode`. No diagnostic — produces architecturally invalid instructions instead of error.

## Suggested Fix

Validate field widths before encoding:

```rust
if rmode > 0b11 {
    return Err(format!("fcvt*: rmode {} exceeds 2-bit field [20:19]", rmode));
}
if opcode > 0b111 {
    return Err(format!("fcvt*: opcode {} exceeds 3-bit field [18:16]", opcode));
}
```

## Regression Property

Failing property: `prop_fcvt_rounding_rejects_oversized_rmode_and_opcode`

```rust
prop_assert!(encode_fcvt_rounding(&[wreg(0), sreg(0)], 4, 0).is_err());
prop_assert!(encode_fcvt_rounding(&[wreg(0), sreg(0)], 0, 8).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/127