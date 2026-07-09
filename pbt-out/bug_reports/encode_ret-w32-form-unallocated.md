# Bug Report: `encode_ret` silently accepts W32 form (UNALLOCATED)

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_ret`
**Severity:** Medium

## Summary

`encode_ret` accepts `w`-register operands. `RET` is defined only with `<Xn>` (64-bit). There is no `<Wn>` form. `ret w0` accepted and encodes as `ret x0` — UNALLOCATED.

## Root Cause

```rust
let (rn, _) = get_reg(operands, 0)?;   // is_64 discarded
let word = 0xd65f0000 | (rn << 5);
```

No width validation.

## Reproduction

**Input:** `ret w0`

**Expected:** `Err` — RET requires 64-bit (X) register

**Actual:** `Ok(Word(0xD65F0020))` — encodes as `ret x0` (W0 number used as X0)

**Minimal failing input:** rn = "w0"

## Impact

32-bit register operands accepted and encoded as 64-bit, producing wrong encodings.

## Suggested Fix

Validate width:

```rust
let (rn, is_64) = get_reg(operands, 0)?;
if !is_64 {
    return Err("RET requires 64-bit (X) register".into());
}
```

## Regression Property

Failing property: `prop_rejects_w32_form`

```rust
prop_assert!(encode_ret(&[Operand::Reg("w0".into())]).is_err());
prop_assert!(encode_ret(&[Operand::Reg("w30".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/156