# Bug Report: `encode_bfi` silently accepts out-of-range `lsb`/`width` (no range validation)

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_bfi`
**Severity:** Medium

## Summary

Even for out-of-range immediates that do **not** underflow the alias arithmetic,
`encode_bfi` performs no validation of `lsb`/`width` against the ARM ARM
operand constraints and returns `Ok` with a 32-bit word whose `immr`/`imms`
fields are architecturally meaningless. An assembler must reject these with
`Err`.

`BFI <Xd>,<Xn>,#<lsb>,#<width>` (ARM ARM, BFI) is constrained:

- 64-bit: `0 <= lsb <= 63`, `1 <= width <= 64 - lsb`
- 32-bit: `0 <= lsb <= 31`, `1 <= width <= 32 - lsb`

## Root Cause

```rust
pub(crate) fn encode_bfi(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let lsb = get_imm(operands, 2)? as u32;    // NO range check
    let width = get_imm(operands, 3)? as u32;  // NO range check
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let reg_width = if is_64 { 64u32 } else { 32u32 };
    let immr = (reg_width - lsb) % reg_width;
    let imms = width - 1;
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22)
        | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))   // always Ok, never validated
}
```

There is no check that `lsb < reg_width`, that `width >= 1`, or that
`lsb + width <= reg_width`. The `as u32` cast and the OR-into-word let
out-of-range `imms` (`= width - 1`) overflow its 6-bit field `[15:10]` into the
`Rn` field `[9:5]`.

## Reproduction

```rust
// lsb == regsize: invalid (lsb must be < regsize), but returns Ok.
let r = encode_bfi(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(64),   // lsb == 64 for the 64-bit form
    Operand::Imm(1),
]);
assert!(r.is_ok());          // BUG: should be Err

// lsb + width > regsize: invalid, but returns Ok with garbage imms.
let r = encode_bfi(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(31),
    Operand::Imm(2),        // 31 + 2 = 33 > 32 for the 32-bit form
]);
assert!(r.is_ok());          // BUG: should be Err

// width > regsize: imms = width - 1 overflows the 6-bit field into Rn.
let r = encode_bfi(&[
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Imm(0),
    Operand::Imm(100),
]);
assert!(r.is_ok());          // BUG: should be Err
```

- Expected: `Err(...)` for each (operand out of range).
- Actual: `Ok(EncodeResult::Word(...))` with a corrupt encoding.

## Impact

Silent mis-assembly: invalid source like `bfi x0, x1, #64, #1` is accepted and
emits a 32-bit word that does not correspond to any valid `BFM` encoding rather
than diagnosing the error. This is the same class of missing-validation defect
documented for the sibling encoders (`encode_bfm_no_range_validation.md`,
`encode_sbfm_no_imm_range_validation.md`, `encode_ubfm_no_range_validation.md`,
`encode_sbfx_no_lsb_width_range_validation.md`).

## Suggested Fix

Validate `lsb`/`width` against the register width before building the word:

```rust
let lsb = get_imm(operands, 2)?;
let width = get_imm(operands, 3)?;
let reg_width: u32 = if is_64 { 64 } else { 32 };
if lsb < 0 || (lsb as u32) >= reg_width {
    return Err(format!("BFI: lsb #{} out of range [0, {})", lsb, reg_width));
}
if width < 1 || (lsb as u32) + (width as u32) > reg_width {
    return Err(format!(
        "BFI: width #{} out of range [1, {}] for lsb #{}",
        width, reg_width - lsb as u32, lsb
    ));
}
```

## Regression Property

Failing property: `prop_encode_bfi_tests::prop_rejects_out_of_range_operands`
(the `lsb==regsize`, `lsb+width>regsize`, and `width>regsize` cases).

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
