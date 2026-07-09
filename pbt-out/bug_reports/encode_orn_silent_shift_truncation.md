# Bug Report: `encode_orn` silently truncates shift amounts above 63 (UNPREDICTABLE)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`
**Severity:** High

## Summary

For 32-bit W-register forms (`sf=0`), shift amounts 32-63 are **UNPREDICTABLE** per ARMv8-A. `encode_orn` masks with `& 0x3F` and accepts these values without validation, emitting malformed instruction words.

## Root Cause

```rust
let imm6 = (shift_amount & 0x3F) << 10;  // no range check for W-register forms
```

## Reproduction

**Input:** `orn w0, w0, w0, lsl #32`

**Expected:** `Err` — ORN shift amount 32 out of range for 32-bit register (max 31)

**Actual:** `Ok(Word(0x0A200000))` — imm6 = 32 accepted and encoded

**Minimal failing input:** rd=0, rn=0, rm=0, amount=32, sk=0

## Impact

UNPREDICTABLE encodings emitted without diagnostic. Silent acceptance masks source-level bugs in hand-written assembly. Same defect class in `encode_eon`, `encode_bic`, `encode_bics`, `encode_logical`, `encode_mvn`, `encode_neg`, `encode_negs`.

## Suggested Fix

Validate shift amount against width before masking:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "orn: shift amount {} out of range for {}-bit register (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```

## Regression Property

Failing property: `orn_negative_contracts` (part b)

```rust
prop_assert!(encode_orn(&[wreg(0), wreg(0), wreg(0)], shift("lsl", 32)]).is_err());
prop_assert!(encode_orn(&[wreg(0), wreg(0), wreg(0)], shift("lsl", 63)]).is_err());
```

## PBT Results (module `data_processing::tests::orn`)

| Property | Result |
|---|---|
| `orn_register_form_field_placement` | PASS |
| `orn_register_form_shift_mapping` | PASS |
| `orn_differs_from_orr_only_in_n_bit` | PASS |
| `orn_neon_vector_form_fields` | PASS |
| `orn_negative_contracts (a)` | PASS |
| `orn_negative_contracts (b)` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/88