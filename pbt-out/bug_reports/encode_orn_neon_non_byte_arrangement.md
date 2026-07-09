# Bug Report: `encode_orn` (NEON) silently accepts non-byte arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_orn` (NEON form)
**Severity:** Medium

## Summary

NEON ORN operates only on `.8b` and `.16b` arrangements. `encode_orn` silently accepts other arrangements like `.4h`, `.8h`, `.2s` by treating them as `.8b` or `.16b`, producing UNALLOCATED encodings.

## Root Cause

```rust
let q = if arr_d == "16b" { 1u32 } else { 0 };  // any non-"16b" arrangement treated as "8b"
```

## Reproduction

**Input:** `orn v0.4h, v1.4h, v2.4h`

**Expected:** `Err` — ORN supports only .8b and .16b arrangements

**Actual:** `Ok(Word(...))` — accepted and encoded with wrong arrangement

**Minimal failing input:** arr_d = "4h" (or "8h", "2s", "4s", "1d", "2d")

## Impact

Non-byte arrangements silently accepted, producing UNALLOCATED encodings. Reference assemblers reject these.

## Suggested Fix

Reject non-byte arrangements:

```rust
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("ORN supports only .8b and .16b arrangements, got {}", arr_d));
}
```

## Regression Property

Failing property: `neon_orn_rejects_non_byte_arrangement`

```rust
prop_assert!(encode_orn(&[neon_reg(0, "4h"), neon_reg(1, "4h"), neon_reg(2, "4h")]).is_err());
prop_assert!(encode_orn(&[neon_reg(0, "2s"), neon_reg(1, "2s"), neon_reg(2, "2s")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/112