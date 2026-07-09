# Bug Report: `encode_ccmp_ccmn` silently truncates out-of-range `imm5` and `nzcv`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_ccmp_ccmn`
**Severity:** High

## Summary

`CCMP/CCMN` immediate form masks operands with `& 0x1F` and `& 0xF` without validation. ARM ARM requires `imm5` in range `0..=31` and `nzcv` in range `0..=15`. Out-of-range and negative immediates silently truncated.

## Root Cause

```rust
let word = ... | ((*imm5 as u32 & 0x1F) << 16) | ... | (*nzcv as u32 & 0xF);
```

No range checks before masking.

## Reproduction

**Input:** `ccmn w0, #32, #16, eq`

**Expected:** `Err` — ccmp/ccmn immediate out of range (0..=31): 32

**Actual:** `Ok(Word(0x3A400000))` — same as `ccmn w0, #0, #0, eq` (32→0, 16→0)

**Other failing inputs:** `ccmp x0, #-1, #0, eq` → encodes as `ccmp x0, #31, #0, eq`

## Impact

Silent miscompilation: out-of-range immediates assembled into incorrect instructions, masking typos/bad codegen with no diagnostic.

## Suggested Fix

Validate before encoding:

```rust
if *imm5 < 0 || *imm5 > 31 {
    return Err(format!("ccmp/ccmn immediate out of range (0..=31): {}", imm5));
}
if *nzcv < 0 || *nzcv > 15 {
    return Err(format!("ccmp/ccmn nzcv out of range (0..=15): {}", nzcv));
}
```

## Regression Property

Failing property: `prop_rejects_out_of_range_immediates`

```rust
prop_assert!(encode_ccmp_ccmn(&[wreg(0), imm(32), imm(16), cond("eq")], false).is_err());
prop_assert!(encode_ccmp_ccmn(&[xreg(0), imm(-1), imm(0), cond("eq")], true).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/21