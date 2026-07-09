# Bug Report: `encode_neg` silently re-encodes `ror` shift as LSL

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** Medium

## Summary

`encode_neg` maps any unrecognized shift kind to `lsl` via the default arm of a `match`. The ARMv8 scalar add/sub shifted-register forms only permit `lsl`, `lsr`, and `asr`; `ror` is not valid here and must be rejected.

## Root Cause

```rust
let st = match kind.as_str() { "lsl" => 0b00u32, "lsr" => 0b01u32, "asr" => 0b10u32, _ => 0b00u32 };  // <-- default lsl for ror
```

## Reproduction

**Input:** `neg x0, x1, ror #5`

**Expected:** `Err` — unsupported shift kind

**Actual:** `Ok` — encodes as `lsl #5`

## Impact

Typos or upstream parser bugs silently produce a different instruction than the assembly source names.

## Suggested Fix

Return `Err` for any shift kind other than `lsl`, `lsr`, or `asr`:

```rust
match kind.as_str() {
    "lsl" => 0b00u32, "lsr" => 0b01u32, "asr" => 0b10u32,
    _ => return Err(format!("neg: invalid shift kind: {} (expected lsl/lsr/asr)", kind)),
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/75