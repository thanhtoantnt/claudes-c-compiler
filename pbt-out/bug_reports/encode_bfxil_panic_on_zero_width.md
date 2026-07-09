# Bug Report: `encode_bfxil` panics on `BFXIL Rd, Rn, #0, #0`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_bfxil`
**Severity:** High

## Summary

`BFXIL Rd, Rn, #0, #0` causes compiler panic due to arithmetic underflow in `lsb + width - 1`. ARM ARM requires `1 <= width <= regsize - lsb`, so `width == 0` must be rejected.

## Root Cause

```rust
let imms = lsb + width - 1;   // line 127
```

Evaluated as `(0 + 0) - 1 = -1` → `0u32 - 1` underflow → panic. No check that `width >= 1`.

## Reproduction

**Input:** `bfxil x0, x1, #0, #0`

**Expected:** `Err` — width 0 out of range (1 <= width <= regsize - lsb)

**Actual:** Panic: `attempt to subtract with overflow` at bitfield.rs:127:16

**Minimal failing input:** lsb = 0, width = 0 (any register width)

## Impact

Any source containing `BFXIL Rd, Rn, #0, #0` (or codegen emitting zero-width extract) crashes compiler instead of producing diagnostic.

## Suggested Fix

Validate before computing:

```rust
if width == 0 || lsb >= regsize || lsb + width > regsize {
    return Err(format!("BFXIL: lsb/width out of range (lsb={}, width={}, regsize={})",
                       lsb, width, regsize));
}
let imms = lsb + width - 1; // now width >= 1, no underflow
```

Or use checked arithmetic: `width.checked_sub(1)`.

## Regression Property

Failing property: `prop_rejects_out_of_range_operands`

```rust
prop_assert!(encode_bfxil(&[xreg(0), xreg(1), imm(0), imm(0)]).is_err());  // width=0 panic
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/138