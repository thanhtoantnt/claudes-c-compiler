# BUG: `encode_neon_shift_imm` ignores `_is_unsigned` — SSHR is unreachable

- **File:** `src/backend/arm/assembler/encoder/neon.rs`
- **Function:** `encode_neon_shift_imm`
- **Severity:** Medium (functional correctness — SSHR/SRSHR/SRSRA family unachievable via this path)

## Description

`encode_neon_shift_imm` has an `_is_unsigned: bool` parameter that is silently
ignored. The U bit (bit 29 in the encoding) is hardcoded to 1, making the
function always emit an unsigned shift (USHR/URSHR/URSRA). The signed
variants (SSHR/SRSHR/SRSRA, U=0) can never be produced regardless of what
the caller passes.

Compare the sibling functions:
- `encode_neon_ushr` — hardcodes U=1 (correct for USHR)
- `encode_neon_sshr` — hardcodes U=0 (correct for SSHR)

`encode_neon_shift_imm` was presumably intended as the common path for both
families, but the `_is_unsigned` switch was never wired.

## PBT-derived evidence

Property `prop_is_unsigned_ignored` (in `mod shift_imm_pbt_tests`):

```rust
proptest! {
    #[test]
    fn prop_is_unsigned_ignored(rd in valid_neon_reg(), rn in valid_neon_reg(),
                                 shift in 1u32..16u32) {
        let ops_u = vec![Operand::Reg(rd, "8h".into()), Operand::Reg(rn, "8h".into()),
                         Operand::Imm(shift as i64)];
        let ops_s = ops_u.clone();
        let word_u = encode_neon_shift_imm(&ops_u, true).unwrap();
        let word_s = encode_neon_shift_imm(&ops_s, false).unwrap();
        prop_assert_eq!(word_u, word_s,   // identical words — U bit never changes
            "is_unsigned=true vs false produced different words");
    }
}
```

Passes (both always produce the same word), confirming the parameter has no effect.

## Secondary finding: panic on out-of-range shift

Additionally, `16 - shift as u32` (before the `& mask`) will underflow (panic
in debug, wrap in release) when `shift > 2 * elem_bits`. No range check is
performed before this arithmetic. The function is not panic-safe for arbitrary
`i64` immediates.

## Suggested fix

```rust
pub(crate) fn encode_neon_shift_imm(
    operands: &[Operand],
    is_unsigned: bool,   // ← use this
) -> Result<EncodeResult, String> {
    // ...
    let u: u32 = if is_unsigned { 1 } else { 0 };  // was: hardcoded 1
    let word = (q << 30) | (u << 29) | /* ... */;
    // Also: range-check shift before arithmetic
    if shift == 0 || shift as u32 > elem_bits {
        return Err(format!("shift {} out of range for arrangement {}", shift, arr));
    }
    // ...
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/83
