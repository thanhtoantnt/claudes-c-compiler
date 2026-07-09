# Bug Report: `encode_shift` panics / silently mis-encodes out-of-range immediate shifts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_shift` (immediate form)
**Severity:** High

## Summary

`encode_shift` accepts any `Operand::Imm` value without range validation, then derives BFM/EXTR `immr`/`imms` via unsigned subtraction. Out-of-range or negative immediates cause panic (debug builds) or silent garbage (release builds).

## Root Cause

```rust
let imm = *imm as u32;          // no validation
let width = 32u32;              // 32-bit (W) register
let immr = (width - imm) % width;   // PANIC if imm > width
let imms = width - 1 - imm;         // PANIC if imm > width-1
```

## Reproduction

**Input:** `lsl w0, w0, #33`

**Expected:** `Err` — shift immediate 33 out of range [0, 31] for 32-bit register

**Actual:** Panic: `attempt to subtract with overflow` at data_processing.rs:810:28

**Minimal failing input:** rd=0, rn=0, st=0, is_64=false, imm=33 (or any imm > 31)

## Impact

- **Crash** (debug builds): single malformed shift-immediate operand aborts compilation
- **Mis-compilation** (release builds): bogus value packed into `immr`/`imms`, emits UNDEFINED encoding with no error

## Suggested Fix

Validate range before field computation:

```rust
if let Some(Operand::Imm(imm_val)) = operands.get(2) {
    let width = if is_64 { 64 } else { 32 };
    let lo = if shift_type == 0b00 { 0i64 } else { 1 };
    let hi = match shift_type {
        0b00 | 0b11 => width as i64 - 1,
        _ => width as i64,
    };
    if *imm_val < lo || *imm_val > hi {
        return Err(format!("shift immediate {} out of range [{}, {}] for {}-bit register",
                           imm_val, lo, hi, width));
    }
}
```

## Regression Property

Failing property: `shift_immediate_rejects_out_of_range`

```rust
prop_assert!(encode_shift(&[wreg(0), wreg(0)], 0b00, 33).is_err());  // LSL W #33 out of range
prop_assert!(encode_shift(&[xreg(0), xreg(0)], 0b00, 64).is_err());  // LSL X #64 out of range
```

## PBT Results (module `data_processing::tests`)

| Property | Result |
|---|---|
| `shift_immediate_bfm_field_placement` | PASS |
| `shift_immediate_ror_extr_field_placement` | PASS |
| `shift_register_form_field_placement` | PASS |
| `shift_immediate_rejects_out_of_range` | **FAIL** (panic) |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/93