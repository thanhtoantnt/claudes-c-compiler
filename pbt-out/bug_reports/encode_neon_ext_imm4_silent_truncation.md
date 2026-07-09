# Bug Report: `encode_neon_ext` silently truncates 4-bit immediate index

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ext`
**Severity:** High

## Summary

`encode_neon_ext` masks immediate index with `& 0xF` without range validation. Immediate field is 3 bits for most arrangements, 1 bit for `.8b`. Out-of-range indices accepted and silently truncated.

## Root Cause

```rust
let imm4 = imm & 0xF;  // no arrangement-specific validation
```

## Reproduction

**Input:** `ext v0.8b, v1.8b, #2`

**Expected:** `Err` — EXT index for .8b must be 0 or 1 (1-bit field)

**Actual:** `Ok(Word(...))` — imm4 = 2 & 0xF = 2, encoded as `#2` (UNALLOCATED)

**Minimal failing input:** arr_d="8b", imm = 2 (or 8, 16, etc.)

## Impact

Invalid indices silently truncated. UNALLOCATED encodings emitted without diagnostic.

## Suggested Fix

Validate against arrangement-specific max before masking:

```rust
let max = match arr_d.as_str() {
    "8b" => 1, "16b" => 3, "4h" => 7, "8h" => 15, "2s" => 31, "4s" => 63,
    _ => return Err(format!("unsupported arrangement: {}", arr_d)),
};
if imm > max {
    return Err(format!("EXT index {} out of range for {}", imm, arr_d));
}
```

## Regression Property

Failing property: `neon_ext_index_range_checked`

```rust
prop_assert!(encode_neon_ext(&[neon_reg(0, "8b"), neon_reg(1, "8b"), 2]).is_err());  // 8b: 0-1
prop_assert!(encode_neon_ext(&[neon_reg(0, "4s"), neon_reg(1, "4s"), 64]).is_err());  // 4s: 0-63
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/85