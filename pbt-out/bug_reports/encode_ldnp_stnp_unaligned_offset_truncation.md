# Bug Report: `encode_ldnp_stnp` silently truncates unaligned immediate offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldnp_stnp`
**Severity:** High

## Summary

`encode_ldnp_stnp` accepts non-aligned offsets and silently rounds down to nearest aligned value. ARMv8-A requires offsets to be aligned to access size (4 for W-registers, 8 for X-registers). Unaligned offsets should be rejected.

## Root Cause

```rust
let imm7 = (*offset >> shift) as u32;  // integer division silently truncates
```

## Reproduction

**Input:** `ldnp w0, w1, [x2, #5]`

**Expected:** `Err` — LDNP offset must be aligned to 4 bytes

**Actual:** `Ok(Word(...))` — offset 5 → imm7 = 5 >> 2 = 1, encodes as `#4`

**Minimal failing input:** offset = 5 (or 1, 2, 3, 6, 7, etc.)

## Impact

Unaligned offsets silently truncated. User expects operation at specific offset but gets different encoding.

## Suggested Fix

Validate alignment before masking:

```rust
let align = if is_64 { 8 } else { 4 };
if offset % align != 0 {
    return Err(format!("LDNP offset must be aligned to {} bytes: {}", align, offset));
}
```

## Regression Property

Failing property: `prop_unaligned_offset_rejected`

```rust
prop_assert!(encode_ldnp_stnp(&[wreg(0), wreg(1), mem_offset(xreg(2), 5)], false).is_err());
prop_assert!(encode_ldnp_stnp(&[xreg(0), xreg(1), mem_offset(xreg(2), 3)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/116