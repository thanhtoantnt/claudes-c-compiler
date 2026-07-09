# Bug Report: `encode_add_sub` silently rewrites unknown extend mnemonic as UXTX

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_add_sub` (extend branch)
**Severity:** Medium

## Summary

The match mapping extend mnemonics to `option` field has a catch-all arm `_ => 0b011` (UXTX). Unrecognized `kind` (typos, fabricated mnemonics, etc.) silently encoded as `uxtx` instead of returning error.

## Root Cause

```rust
let option = match kind.as_str() {
    "uxtb" => 0b000u32,
    "uxth" => 0b001,
    "uxtw" => 0b010,
    "uxtx" => 0b011,
    "sxtb" => 0b100,
    "sxth" => 0b101,
    "sxtw" => 0b110,
    "sxtx" => 0b111,
    _ => 0b011, // default UXTX/LSL        // <-- FINDING
};
```

## Reproduction

**Input:** `add x0, x1, x2, foo`

**Expected:** `Err` — expected 'sxtx' 'uxtx' or 'lsl' with optional integer in range [0, 4]

**Actual:** `Ok(Word(...))` — silently encoded as `uxtx`

**Minimal failing input:** kind = "uxzz"

## Impact

Silent acceptance of typos and invalid extend mnemonics. LLVM rejects `foo`; this encoder accepts and emits UXTX bits.

## Suggested Fix

Replace catch-all with explicit error:

```rust
_ => return Err(format!("invalid extend kind '{}'; expected uxtb/uxth/uxtw/uxtx/sxtb/sxth/sxtw/sxtx (or lsl)", kind)),
```

Handle `lsl` synonym explicitly: `lsl` → `uxtx` on 64-bit, `uxtw` on 32-bit.

## Regression Property

Failing property: `add_extended_register_rejects_unknown_extend_kind`

```rust
prop_assert!(encode_add_sub_extended(&[xreg(0), xreg(1), xreg(2)], "foo", 0).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/125