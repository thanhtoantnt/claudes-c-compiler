# Bug Report: `encode_neon_across_long` accepts unallocated `size=11` arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_across_long`
**Severity:** Low

## Summary

`encode_neon_across_long` (SADDLV/UADDLV) converts the source arrangement to a `(Q, size)` pair without validating that `size` is architecturally allocated. Per ARMv8-A, SADDLV/UADDLV only allows `size ∈ {0b00, 0b01, 0b10}`. `size == 0b11` (for `.1d`/`.2d` sources) is UNALLOCATED but the encoder silently emits it.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_n)?;  // .1d/.2d -> size=0b11
let word = ... | (size << 22) | ...;           // no validation
```

## Reproduction

**Input:** `saddlv d0, v0.1d`

**Expected:** `Err` — SADDLV/UADDLV does not support .1d (size=0b11 UNALLOCATED)

**Actual:** `Ok(Word(0x0EF03800))` — UNALLOCATED encoding emitted

**Minimal failing input:** arr = "1d" (or "2d")

## Impact

Silent emission of unallocated encoding. Runtime UC exception or mis-decode as unrelated instruction.

## Suggested Fix

Reject `size == 0b11`:

```rust
let (q, size) = neon_arr_to_q_size(&arr_n)?;
if size == 0b11 {
    return Err(format!("saddlv/uaddlv: unsupported source arrangement {} (size=0b11 is UNALLOCATED)", arr_n));
}
```

## Regression Property

Failing property: `across_long_rejects_unallocated_size`

```rust
prop_assert!(encode_neon_across_long(&[dreg(0), neon_reg(0, "1d")], 0, 0b00011).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/44