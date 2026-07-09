# Bug Report: `encode_ldp_stp` silently wraps out-of-range imm7 offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldp_stp`
**Severity:** High

## Summary

`encode_ldp_stp` encodes offset as `(*offset >> shift) & 0x7F` without validating signed 7-bit scaled range. Out-of-range byte offsets accepted and wrap to different signed imm7 value, accessing wrong address.

## Root Cause

```rust
let imm7 = (*offset >> shift) & 0x7F;  // no range check
```

## Reproduction

**Input:** `stp w0, w1, [x2, #256]`

**Expected:** `Err` — STP offset out of range: 256 (valid: -256 to 252)

**Actual:** `Ok(Word(...))` — imm7 = -64, encodes as `#-256` (wrong offset)

**Minimal failing input:** is_w_reg = true, offset = 256 (or 257, 512, etc.)

## Impact

Silent miscompilation: out-of-range offsets accepted and access different address than written. Hard to debug.

## Suggested Fix

Validate signed scaled range before masking:

```rust
let scale = if is_64 { 8 } else { 4 };
if offset < -(64 * scale) || offset > (63 * scale) {
    return Err(format!("STP/LDP offset out of range: {} (valid: {} to {})", 
                       offset, -(64 * scale), 63 * scale));
}
```

## Regression Property

Failing property: `prop_imm7_range_violation_rejects`

```rust
prop_assert!(encode_ldp_stp(&[wreg(0), wreg(1), mem_offset(xreg(2), 256)], false).is_err());
prop_assert!(encode_ldp_stp(&[wreg(0), wreg(1), mem_offset(xreg(2), -260)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/117