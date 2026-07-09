# Bug Report: `encode_neon_mla` accepts UNALLOCATED doubleword arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mla`
**Severity:** Medium

## Summary

`encode_neon_mla` accepts doubleword arrangements (`.2d`) via `neon_arr_to_q_size` without validation. ARMv8-A NEON MLA has no `.2d` form — valid arrangements are `.8b`, `.16b`, `.4h`, `.8h`, `.2s`, `.4s`. UNALLOCATED encodings emitted without diagnostic.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;  // maps "2d" → (1, 0b11) with no rejection
```

## Reproduction

**Input:** `mla v0.2d, v1.2d, v2.2d`

**Expected:** `Err` — MLA arrangement not supported: 2d (valid: 8b, 16b, 4h, 8h, 2s, 4s)

**Actual:** `Ok(Word(...))` — accepted with size=0b11, UNALLOCATED encoding

**Minimal failing input:** arr_d="2d", arr_n="2d", arr_m="2d"

## Impact

Invalid `.2d` arrangements accepted, producing UNALLOCATED encodings. Reference assemblers reject this.

## Suggested Fix

Reject doubleword arrangements:

```rust
let valid_arrangements = ["8b", "16b", "4h", "8h", "2s", "4s"];
if !valid_arrangements.contains(&arr_d.as_str()) {
    return Err(format!("MLA arrangement not supported: {} (valid: 8b, 16b, 4h, 8h, 2s, 4s)", arr_d));
}
```

## Regression Property

Failing property: `neon_mla_rejects_doubleword`

```rust
prop_assert!(encode_neon_mla(&[neon_reg(0, "2d"), neon_reg(1, "2d"), neon_reg(2, "2d")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/88