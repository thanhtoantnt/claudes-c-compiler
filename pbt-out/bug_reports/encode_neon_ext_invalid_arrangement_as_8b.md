# Bug Report: `encode_neon_ext` accepts `.8b` as valid arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_ext`
**Severity:** Medium

## Summary

`encode_neon_ext` accepts `arr_d = "8b"` for both registers, but ARMv8-A NEON EXT has no `.8b` arrangement. Valid arrangements are `.16b`, `.4h`, `.8h`, `.2s`, `.4s`. The encoder should reject `.8b` as invalid.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;  // no 8b-specific rejection
```

`neon_arr_to_q_size` maps `"8b"` → `(0, 0b00)` but EXT encoding requires different handling.

## Reproduction

**Input:** `ext v0.8b, v1.8b, #0`

**Expected:** `Err` — EXT arrangement not supported: 8b (valid: 16b, 4h, 8h, 2s, 4s)

**Actual:** `Ok(Word(...))` — accepted with size=00, UNALLOCATED encoding

**Minimal failing input:** arr_d="8b", arr_n="8b"

## Impact

Invalid `.8b` arrangement accepted, producing UNALLOCATED encodings. Reference assemblers reject this.

## Suggested Fix

Explicitly reject `.8b` in `encode_neon_ext`:

```rust
let valid_arrangements = ["16b", "4h", "8h", "2s", "4s"];
if !valid_arrangements.contains(&arr_d.as_str()) {
    return Err(format!("EXT arrangement not supported: {} (valid: 16b, 4h, 8h, 2s, 4s)", arr_d));
}
```

## Regression Property

Failing property: `neon_ext_rejects_invalid_arrangements`

```rust
prop_assert!(encode_neon_ext(&[neon_reg(0, "8b"), neon_reg(1, "8b"), 0]).is_err());
prop_assert!(encode_neon_ext(&[neon_reg(0, "2d"), neon_reg(1, "2d"), 0]).is_err());  // doubleword not supported
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/86