# Bug Report: `encode_ldnp_stnp` silently wraps out-of-range imm7 offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldnp_stnp`
**Severity:** High

## Summary

`encode_ldnp_stnp` encodes offset as `(*offset >> shift) & 0x7F` without validating signed 7-bit scaled range. Out-of-range byte offsets accepted and wrap to different signed imm7 value, accessing wrong address.

## Root Cause

```rust
let imm7 = (*offset >> shift) & 0x7F;  // no range check
```

## Reproduction

**Input:** `stnp w0, w1, [x2, #256]`

**Expected:** `Err` — STNP offset out of range: 256 (valid: -256 to 252)

**Actual:** `Ok(Word(...))` — imm7 = -64, encodes as `#-256` (wrong offset)

**Minimal failing input:** is_w_reg = true, offset = 256 (or 257, 512, etc.)

## Impact

Silent miscompilation: out-of-range offsets accepted and access different address than written. Hard to debug.

## Suggested Fix

Validate signed scaled range before masking:

```rust
let scale = if is_64 { 8 } else { 4 };
if offset < -(64 * scale) || offset > (63 * scale) {
    return Err(format!("STNP offset out of range: {} (valid: {} to {})", 
                       offset, -(64 * scale), 63 * scale));
}
```

## Regression Property

Failing property: `prop_negative_imm7_range_violation_rejects`

```rust
prop_assert!(encode_ldnp_stnp(&[wreg(0), wreg(1), mem_offset(xreg(2), 256)], false).is_err());
prop_assert!(encode_ldnp_stnp(&[wreg(0), wreg(1), mem_offset(xreg(2), -260)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/115