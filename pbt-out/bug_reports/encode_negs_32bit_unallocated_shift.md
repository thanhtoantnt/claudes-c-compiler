# Bug Report: `encode_negs` silently accepts 32-bit UNALLOCATED shift

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

ARMv8-A defines `NEGS` with shift kind field `1 00` (LSL) only; values `01`, `10`, `11` are UNALLOCATED. `encode_negs` masks shift amount with `& 0x3` without validating shift kind, accepting invalid shifts like `lsr`, `asr`, `ror`.

## Root Cause

```rust
let st = *amount & 0x3;  // no shift kind validation
```

## Reproduction

**Input:** `negs x0, x1, lsr #5`

**Expected:** `Err` — NEGS shift must be LSL only

**Actual:** `Ok(Word(...))` — st = 5 & 0x3 = 1, encoded as LSR (UNALLOCATED)

**Minimal failing input:** shift_kind = "lsr" (or "asr", "ror"), amount = 0, 1, 2, or 3

## Impact

UNALLOCATED encodings emitted without diagnostic. User may expect valid shift but gets architecturally undefined instruction.

## Suggested Fix

Reject non-LSL shift kinds:

```rust
match kind.as_str() {
    "lsl" => { /* proceed */ }
    _ => return Err(format!("negs: invalid shift kind: {} (LSL only)", kind)),
}
```

## Regression Property

Failing property: `negs_rejects_non_lsl_shift`

```rust
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("lsr", 5)], false).is_err());
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("asr", 5)], true).is_err());
prop_assert!(encode_negs(&[xreg(0), xreg(1), shift("ror", 5)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/79