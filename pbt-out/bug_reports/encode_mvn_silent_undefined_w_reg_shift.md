# Bug Report: `encode_mvn` silently accepts undefined W-register shifts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mvn`
**Severity:** Medium

## Summary

For 32-bit W-register shifts, ARMv8-A specifies shift kinds `LSL`, `LSR`, `ASR`, `ROR` are UNALLOCATED. `encode_mvn` silently accepts these invalid shift kinds and encodes them as LSL via default match arm, producing UNALLOCATED encodings without diagnostic.

## Root Cause

```rust
let st = match kind.as_str() { "lsl" => 0b00u32, "lsr" => 0b01u32, "asr" => 0b10u32, _ => 0b00u32 };  // default LSL
```

## Reproduction

**Input:** `mvn w0, w1, ror #5`

**Expected:** `Err` — MVN W: shift kind ROR is UNALLOCATED

**Actual:** `Ok(Word(...))` — encoded as LSL

**Minimal failing input:** is_64 = false, kind = "ror" (or "asr")

## Impact

UNALLOCATED encodings emitted without diagnostic. User expectation of ROR/ASR silently coerced to LSL.

## Suggested Fix

Reject invalid W-register shift kinds:

```rust
let st = match kind.as_str() {
    "lsl" => 0b00u32,
    _ => return Err(format!("mvn {}: shift kind {} is UNALLOCATED for W registers (valid: LSL)", kind, kind)),
};
```

## Regression Property

Failing property: `mvn_rejects_undefined_w_shift_kinds`

```rust
prop_assert!(encode_mvn(&[wreg(0), wreg(1), shift("ror", 5)]).is_err());
prop_assert!(encode_mvn(&[wreg(0), wreg(1), shift("asr", 5)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/110