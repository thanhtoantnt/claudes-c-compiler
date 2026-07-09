# Bug Report: `encode_fcvt_precision` emits UNALLOCATED encodings for same-precision conversions

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fcvt_precision`
**Severity:** High

## Summary

`encode_fcvt_precision` derives `ftype` (source precision) and `opc` (destination precision) independently from operand prefixes but never checks they differ. When source and destination precisions match (`Sd,Sn`, `Dd,Dn`, `Hd,Hn`), it emits `ftype == opc`, which is **UNALLOCATED** in ARMv8-A. `FCVT D0, D0` yields `0x1E604000` which is exactly `FMOV Dd, Dn` — silent transformation to a different instruction.

## Root Cause

```rust
let ftype = if src_name.starts_with('d') { 0b01 } else { 0b00 };
let opc = if dst_name.starts_with('d') { 0b01 } else { 0b00 };
// No check: ftype == opc is unallocated
```

## Reproduction

**Input:** `FCVT S0, S0`

**Expected:** `Err` — FCVT requires different source and destination precisions

**Actual:** `Ok(Word(0x1E224000))` — UNALLOCATED encoding

**Minimal failing input:** kind = 0, n = 0

## Impact

Silent success for illegal instruction. `FCVT D0, D0` emits exact `FMOV D0, D0` encoding. Consumers receive incoherent word with no diagnostic.

## Suggested Fix

Reject equal precisions:

```rust
if ftype == opc {
    return Err(format!(
        "fcvt: source and dest precision must differ (both {:?})",
        dst_name.chars().next().unwrap()
    ));
}
```

## Regression Property

Failing property: `prop_fcvt_precision_rejects_same_precision`

```rust
prop_assert!(encode_fcvt_precision(&[sreg(0), sreg(0)]).is_err());  // S→S
prop_assert!(encode_fcvt_precision(&[dreg(0), dreg(0)]).is_err());  // D→D
prop_assert!(encode_fcvt_precision(&[href(0), href(0)]).is_err());  // H→H
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/132