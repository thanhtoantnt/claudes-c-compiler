# Bug Report: `encode_bfi` panics on integer underflow for out-of-range `lsb`/`width`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_bfi`
**Severity:** High

## Summary

`encode_bfi` computes two `u32` subtractions unconditionally. When `width == 0`
the term `width - 1` underflows, and when `lsb > reg_width` the term
`reg_width - lsb` underflows. Both panic in debug builds (`attempt to subtract
with overflow`) and wrap silently in release builds. Negative immediates also
trigger the panic, because `get_imm(...)? as u32` wraps a negative `i64` into a
value larger than `reg_width`, which then underflows the subtraction.

An assembler must reject invalid operands with a clean `Err`; panicking aborts
the process and (in release) silently emits a corrupt word.

## Root Cause

```rust
pub(crate) fn encode_bfi(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;      // negatives wrap to > reg_width
    let width = get_imm(operands, 3)? as u32;    // 0 is accepted unvalidated
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let reg_width = if is_64 { 64u32 } else { 32u32 };
    let immr = (reg_width - lsb) % reg_width;    // PANICS when lsb > reg_width
    let imms = width - 1;                        // PANICS when width == 0
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22)
        | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

Both `reg_width - lsb` and `width - 1` are plain `u32` subtractions with no guard.
`BFI`'s alias arithmetic (`immr = (-lsb) MOD regsize`) is what *adds* the
`reg_width - lsb` term that the raw `encode_bfm` does not have, so `encode_bfi`
is uniquely exposed.

## Reproduction

```rust
// debug build: panics ("attempt to subtract with overflow")
encode_bfi(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(0),
    Operand::Imm(0),   // imms = width - 1 = 0u32 - 1  ->  PANIC
]);

// also panics (lsb > reg_width -> reg_width - lsb underflows):
encode_bfi(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(100),
    Operand::Imm(1),
]);
```

Proptest output:

```
Test failed: width=0 (imms underflow): lsb=0 width=0 should be Err but PANICKED
minimal failing input: is_64 = false, over_lsb = 64, over_width = 65, neg = -3
```

- Expected: `Err(...)` (invalid immediate).
- Actual: panic in debug; in release, a word with `imms = u32::MAX` (overflowing
  the 6-bit `imms` field into `Rn`/`Rd`).

## Impact

A single malformed `bfi` — including one produced by a downstream pass or
constant folding that did not pre-range-check its immediates — crashes the
assembler in debug builds. In release builds the same input silently produces a
garbage instruction word.

## Suggested Fix

Validate the immediates and/or use checked arithmetic so no input can panic:

```rust
let lsb = get_imm(operands, 2)?;
let width = get_imm(operands, 3)?;
let reg_width: u32 = if is_64 { 64 } else { 32 };
if lsb < 0 || (lsb as u32) > reg_width {
    return Err(format!("BFI: lsb #{} out of range [0, {}]", lsb, reg_width));
}
if width < 1 || (width as u32) > reg_width {
    return Err(format!("BFI: width #{} out of range [1, {}]", width, reg_width));
}
let lsb = lsb as u32;
let width = width as u32;
let immr = (reg_width - lsb) % reg_width; // now provably lsb in [0, reg_width]
let imms = width - 1;                     // now provably width >= 1
```

## Regression Property

Failing property: `prop_encode_bfi_tests::prop_rejects_out_of_range_operands`
(the `width=0`, `lsb>regsize`, `negative lsb`, and `negative width` cases).

```rust
#[test]
fn prop_rejects_out_of_range_operands(
    is_64 in any::<bool>(),
    over_lsb in 64u32..=1023u32,
    over_width in 65u32..=1023u32,
    neg in (-1024i64)..(-1i64),
) {
    let reg_width: u32 = if is_64 { 64 } else { 32 };
    let cases: &[(i64, i64, &str)] = &[
        (0, 0, "width=0 (imms underflow)"),
        (reg_width as i64, 1, "lsb==regsize"),
        (over_lsb as i64, 1, "lsb>regsize (immr underflow)"),
        ((reg_width - 1) as i64, 2, "lsb+width>regsize"),
        (0, over_width as i64, "width>regsize"),
        (neg, 1, "negative lsb"),
        (0, neg, "negative width"),
    ];
    for &(lsb, width, label) in cases {
        let ops = vec![
            Operand::Reg(reg_name(0, is_64)),
            Operand::Reg(reg_name(1, is_64)),
            Operand::Imm(lsb),
            Operand::Imm(width),
        ];
        let got = panic::catch_unwind(panic::AssertUnwindSafe(|| encode_bfi(&ops)));
        match got {
            Ok(Ok(w)) => prop_assert!(false,
                "{}: lsb={} width={} should be Err, got Ok({:?})", label, lsb, width, w),
            Ok(Err(_)) => {}
            Err(_) => prop_assert!(false,
                "{}: lsb={} width={} should be Err but PANICKED", label, lsb, width),
        }
    }
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/136
