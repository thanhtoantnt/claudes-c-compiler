# Bug Report: `encode_fneg` silently accepts illegal operand types

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fneg`
**Severity:** High

## Summary

`encode_fneg` never validates operand bank. FNEG operates only on FP/SIMD registers (`S`/`D`/`H`). GP (W/X) operands accepted and silently re-encoded as FP register numbers.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;  // no bank check
let (rm, _) = get_reg(operands, 1)?;  // no bank check
```

`get_reg` accepts any register prefix including GP `w`/`x`.

## Reproduction

**Input:** `fneg w0, w1`

**Expected:** `FNEG requires FP/SIMD operands`

**Actual:** `Ok(Word(...))` — GP register numbers used as FP registers

**Other failing inputs:** `fneg d0, d0` (valid), `fneg s0, s0` (valid)

## Impact

GP-bank operands accepted and encoded as FP, producing silently wrong instructions.

## Suggested Fix

Validate operand bank before encoding:

```rust
let names: Vec<_> = (0..2).map(|i| match &operands[i] {
    Operand::Reg(r) => r.to_lowercase(),
    _ => return Err("FNEG requires register operands".into()),
}).collect();
if !names.iter().all(|n| is_fp_reg(n)) {
    return Err("FNEG requires FP/SIMD operands".into());
}
```

## Regression Property

Failing property: `prop_fneg_rejects_non_fp_operands`

```rust
prop_assert!(encode_fneg(&[wreg(0), wreg(1)]).is_err());  // GP bank
prop_assert!(encode_fneg(&[vreg_arr(0, "8b"), vreg_arr(1, "8b")]).is_err());  // NEON form only
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/226