# Bug Report: `encode_add_sub` extended-register shift silently truncated (`& 0x7`)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_add_sub` (extend branch)
**Severity:** Medium

## Summary

The optional shift for ADD/SUB extended-register form (`imm3`, bits 12:10) is masked with `& 0x7` instead of range-checking. ARMv8 ARM restricts shift to **0..=4**; values 5–7 are UNDEFINED, yet encoder silently accepts them.

## Root Cause

```rust
if let Some(Operand::Extend { kind, amount }) = operands.get(3) {
    let option = match kind.as_str() { /* ... */ };
    let imm3 = *amount & 0x7;                       // <-- silently truncates
    let word = ... | (imm3 << 10) | ...;
    return Ok(EncodeResult::Word(word));
}
```

## Reproduction

**Input:** `add x0, x1, x2, uxtx #5`

**Expected:** `Err` — shift amount must be in range [0, 4]

**Actual:** `Ok(Word(...))` — silently truncates to `imm3 = 1`

**Minimal failing input:** amount = 5

## Impact

Silent acceptance of undefined shift values. Values 5–7 (and any amount ≥8 whose masked value lands in 0–7) accepted without diagnostic.

## Suggested Fix

Validate range before encoding:

```rust
if *amount > 4 {
    return Err(format!("add/sub extended shift amount {} must be in range [0, 4]", amount));
}
let imm3 = *amount;
```

## Regression Property

Failing property: `add_uxtx_shift_above_4_must_be_rejected`

```rust
prop_assert!(encode_add_sub_extended(&[xreg(0), xreg(1), xreg(2)], "uxtx", 5).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/124