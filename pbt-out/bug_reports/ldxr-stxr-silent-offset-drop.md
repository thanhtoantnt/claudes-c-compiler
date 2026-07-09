# Bug Report: `encode_ldxr_stxr` silently drops immediate offset

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldxr_stxr`
**Severity:** High

## Summary

`encode_ldxr_stxr` accepts any immediate offset in memory operand and silently discards it. LDXR/STXR encoding has **no immediate-offset field** — non-zero offset is unrepresentable and must be rejected.

## Root Cause

```rust
let rn = match operands.get(1) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
    // offset never inspected — silently dropped
};
```

## Reproduction

**Input:** `stxr w0, x1, [x2, #8]`

**Expected:** `Err` — LDXR/STXR does not support immediate offset

**Actual:** `Ok(Word(...))` — offset silently dropped, encodes as `stxr w0, x1, [x2]`

## Impact

Incorrect code generation with wrong effective address. No diagnostic.

## Suggested Fix

Reject non-zero offsets:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!("ldxr/stxr does not support offset (got #{})", offset));
    }
    parse_reg_num(base).ok_or("invalid base")?
}
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected`

```rust
prop_assert!(encode_ldxr_stxr(&[wreg(0), xreg(1), mem_offset(xreg(2), 8)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/181