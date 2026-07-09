# Bug Report: `encode_negs` silently accepts `ror` shift (re-encoded as LSL)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

ARMv8-A defines `NEGS` only with LSL shift; values 01, 10, 11 in shift kind field are UNALLOCATED. `encode_negs` masks shift amount with `& 0x3` without validating shift kind, accepting invalid `ror` (and `lsr`, `asr`).

## Root Cause

```rust
let st = *amount & 0x3;  // no shift kind validation
```

## Reproduction

**Input:** `negs x0, x1, ror #16`

**Expected:** `Err` — NEGS shift must be LSL only (kind 00)

**Actual:** `Ok(Word(...))` — st = 16 & 0x3 = 0, encoded as LSL (ror silently dropped)

**Minimal failing input:** shift_kind = "ror" (or "lsr", "asr"), amount = 16 (or 0, 32, 48)

## Impact

UNALLOCATED encodings emitted without diagnostic. Shift kind silently coerced to LSL.

## Suggested Fix

Reject non-LSL shift kinds:

```rust
match kind.as_str() {
    "lsl" => { /* proceed */ }
    _ => return Err(format!("negs: invalid shift kind: {} (LSL only)", kind)),
}
```

## Regression Property

Failing property: `negs_rejects_ror_shift`

```rust
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("ror", 16)], true).is_err());
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("lsr", 16)], true).is_err());
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("asr", 16)], true).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/81