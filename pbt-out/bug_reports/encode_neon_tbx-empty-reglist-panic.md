# Bug Report: `encode_neon_tbx` panics on empty register list

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** Medium

## Summary

`encode_neon_tbx` indexes `regs[0]` without checking emptiness. An empty `RegList` panics instead of returning `Err`.

## Root Cause

```rust
Operand::RegList(regs) => {
    let first_reg = match &regs[0] { ... };  // panics if empty
}
```

## Reproduction

**Input:** `tbx` with empty table list

**Expected:** `Err`

**Actual:** panic: index out of bounds

## Impact

Malformed AST crashes the assembler instead of a recoverable diagnostic.

## Suggested Fix

```rust
if regs.is_empty() {
    return Err("tbx: table register list is empty".into());
}
```

## Regression Property

Failing property: `tbx_rejects_empty_register_list`

```rust
prop_assert!(encode_neon_tbx(&[va(0,"16b"), RegList(vec![]), va(2,"16b")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/245
