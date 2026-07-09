# Bug Report: `encode_neon_two_misc_narrow` accepts out-of-range `u_bit`/`opcode`, silently corrupting the encoding

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_two_misc_narrow`
**Severity:** Medium

## Summary

`encode_neon_two_misc_narrow(operands, u_bit, opcode, is_high)` performs **no
range validation** on its `u_bit` and `opcode` parameters. Per the ARMv8 ARM,
the "Advanced SIMD two-register miscellaneous (narrow)" layout is
`0 Q U 01110 size 10000 opcode 10 Rn Rd`, which requires `U` to be a single bit
(0 or 1) and `opcode` to fit in 5 bits (0..=0x1F). The encoder blindly OR-shifts
the caller's values into the word, so out-of-range arguments overflow into
neighbouring fields and emit a **different, corrupted instruction** instead of
returning `Err`.

Concretely:
- `u_bit == 2` shifts into bit 30 (the **Q** bit) — a Q=0 instruction (e.g.
  `XTN`) silently becomes its Q=1 counterpart (`XTN2`).
- `u_bit >= 4` shifts into bit 31, which the encoding requires to be `0`, so
  the emitted word is no longer a valid `0`-prefixed SIMD instruction at all.
- `opcode > 0x1F` overflows into the constant `10000` field (bits 21-17) and
  then the `01110` field (bits 28-24), mangling the opcode group.

## Root Cause

No bounds check before assembly; the parameters are used directly:

```rust
pub(crate) fn encode_neon_two_misc_narrow(
    operands: &[Operand], u_bit: u32, opcode: u32, is_high: bool,
) -> Result<EncodeResult, String> {
    // ... (only operand count + arrangement are validated) ...
    let q = if is_high { 1u32 } else { 0 };
    // 0 Q U 01110 size 10000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
        | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`u_bit << 29` and `opcode << 12` place any `u32` value, not a masked bit / 5-bit
field.

## Reproduction

```text
encode_neon_two_misc_narrow(&[RegArrangement("v0","8b"), RegArrangement("v0","8h")],
                            u_bit = 2, opcode = 0b10010 /* XTN */, is_high = false)
  → Ok(Word(0x4E21_2800))        // expected Err; actual is the XTN2 (Q=1) word

# verify: the SAME call with u_bit = 1 (valid, UQXTN) yields 0x2E21_2800,
# and with u_bit = 0 (valid, XTN) yields 0x0E21_2800.
# u_bit = 2 thus silently flips Q on, turning XTN into XTN2.
```

The property-based test shrinks the minimal failing input to
`rd=0, rn=0, arr_n="8h", bad_u=2, bad_opcode=32, is_high=false`.

## Impact

A malformed/garbage operand path (or a future caller computing `u_bit`/`opcode`
incorrectly) produces a **silently wrong instruction word** with no diagnostic.
Because `u_bit=2` and `is_high=true` collide on bit 30, and `u_bit>=4` breaks the
mandatory `0` top bit, the encoder can emit words that disassemble as a
different NEON op (e.g. `XTN` → `XTN2`) or as an unallocated/invalid encoding.
In an assembler backend this is silent mis-compilation. The defect is shared by
the sibling non-narrow encoder `encode_neon_two_misc` (same pattern).

## Suggested Fix

Validate the field widths up front and reject out-of-range values:

```rust
if u_bit > 1 {
    return Err(format!("NEON two-reg narrow: u_bit must be 0 or 1, got {}", u_bit));
}
if opcode > 0x1F {
    return Err(format!("NEON two-reg narrow: opcode must be 5-bit (0..=0x1F), got {}", opcode));
}
```

(Equivalently mask with `& 1` / `& 0x1F`, but rejecting is preferable to
silent wrapping, per the encoder's `Result<_, String>` contract.)

## Regression Property

Failing property: `prop_narrow_rejects_out_of_range_u_bit_and_opcode`

```rust
proptest! {
    #[test]
    fn prop_narrow_rejects_out_of_range_u_bit_and_opcode(
        rd in 0u32..=31, rn in 0u32..=31,
        arr_n in prop_oneof![Just("8h"), Just("4s"), Just("2d")],
        bad_u in 2u32..=0xFF, bad_opcode in 0x20u32..=0xFFFF,
        is_high in any::<bool>(),
    ) {
        let ops = vec![vreg_arr(rd, arr_n), vreg_arr(rn, arr_n)];
        prop_assert!(encode_neon_two_misc_narrow(&ops, bad_u, 0, is_high).is_err());
        prop_assert!(encode_neon_two_misc_narrow(&ops, 0, bad_opcode, is_high).is_err());
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/3
