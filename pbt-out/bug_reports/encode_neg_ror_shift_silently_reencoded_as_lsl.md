# Bug Report: `encode_neg` `ror` shift silently re-encoded as LSL

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** Medium

## Summary

`encode_neg` maps unrecognized shift kind to `lsl` via default arm of `match`. ARMv8 scalar add/sub shifted-register forms permit only `lsl`, `lsr`, `asr`; `ror` invalid and must be rejected.

## Root Cause

```rust
let st = match kind.as_str() { "lsl" => 0b00u32, "lsr" => 0b01u32, "asr" => 0b10u32, _ => 0b00u32 };
```

## Reproduction

**Input:** `neg x0, x1, ror #5`

**Expected:** `Err` — neg: invalid shift kind: ror (expected lsl/lsr/asr)

**Actual:** `Ok` — encodes as `lsl #5`

## Impact

Typos or parser bugs silently produce different instruction than assembly source names.

## Suggested Fix

Reject unknown shift kinds:

```rust
let st = match kind.as_str() {
    "lsl" => 0b00u32,
    "lsr" => 0b01u32,
    "asr" => 0b10u32,
    _ => return Err(format!("neg: invalid shift kind: {} (expected lsl/lsr/asr)", kind)),
};
```

## Regression Property

Failing property: `neg_rejects_ror_shift`

```rust
prop_assert!(encode_neg(&[xreg(0), xreg(1), shift("ror", 5)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/77