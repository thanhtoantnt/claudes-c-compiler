# Bug Report: `encode_neon_sri` silently accepts out-of-range shift, emits UNDEFINED encoding

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sri`
**Severity:** High

## Summary

`encode_neon_sri` encodes the AArch64 NEON `SRI Vd.T, Vn.T, #shift` (Shift Right
and Insert) instruction. Per the ARMv8-A ARM ("SRI (vector)"), the shift
immediate is constrained to `1 <= shift <= esize`, and the `immh` field (the top
nibble of `immh:immb`) **must not be `0b0000`** — an all-zero `immh` is
UNDEFINED. The encoder performs **no shift-range validation**: it subtracts the
shift from `2*esize` under a per-width mask, so any out-of-range shift is
silently wrapped into a word whose `immh` is `0b0000`, producing an
architecturally-UNDEFINED encoding instead of returning `Err`.

Verified independently with `llvm-mc-14`: `sri v0.8b, v1.8b, #0` and
`sri v0.8b, v1.8b, #9` both fail with
`immediate must be an integer in range [1, 8]`.

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;   // i64 -> u32, no sign/range check
...
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,
    "4h" | "8h"  => (32 - shift) & 0x1F,
    "2s" | "4s"  => (64 - shift) & 0x3F,
    "2d"         => (128 - shift) & 0x7F,
    _ => return Err(format!("unsupported sri arrangement: {}", arr_d)),
};
```

There is no check that `1 <= shift <= esize`. For `shift == 0` the field becomes
`2*esize & mask = 0` (immh `0000`); for `esize < shift <= 2*esize` the mask wraps
the subtraction back to a value whose top nibble is `0000`. Both cases yield
`immh == 0b0000`, which the ARM ARM marks UNDEFINED. (Shifts beyond `2*esize`
and negative immediates additionally cause a debug-mode arithmetic-overflow
panic — see the sibling report `encode_neon_sri_panic_on_negative_shift.md`.)

## Reproduction

```
Input:  sri v0.8b, v1.8b, #0          (esize = 8; shift 0 is out of [1, 8])
Actual: Ok(EncodeResult::Word(788546560))   == 0x2F004420   (immh:immb = 0b0000000)
Expected: Err                               (llvm-mc: "immediate must be an integer in range [1, 8]")
```

Decoding `0x2F004420`: `immh = 0000` (bits 22-19) → UNDEFINED encoding.

`llvm-mc` confirmation of the valid range:

```
$ printf 'sri v0.8b, v1.8b, #0\nsri v0.8b, v1.8b, #9\n' \
  | llvm-mc-14 -assemble -arch=aarch64 -mattr=+neon
<stdin>:1:19: error: immediate must be an integer in range [1, 8].   # #0
<stdin>:2:19: error: immediate must be an integer in range [1, 8].   # #9
```

## Impact

The encoder emits a bit pattern no conforming AArch64 core will execute as
`SRI`; a downstream assembler/linker/disassembler must treat it as UNDEFINED.
This is a silent mis-encoding with no diagnostic: any caller that hands an
unvalidated shift (e.g. `#0`, or a value off-by-one past the element size) gets
a well-formed `Ok(Word(..))` instead of `Err`, masking the defect all the way to
the object file.

## Suggested Fix

Validate the shift against the element size (and reject negative immediates,
since `get_imm` returns `i64`) before computing the field:

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 1 {
    return Err(format!("sri: shift must be in [1, esize], got {}", shift_i));
}
let shift = shift_i as u32;
let (esize, immh_immb) = match arr_d.as_str() {
    "8b" | "16b" => (8u32,  16u32 - shift),
    "4h" | "8h"  => (16u32, 32u32 - shift),
    "2s" | "4s"  => (32u32, 64u32 - shift),
    "2d"         => (64u32, 128u32 - shift),
    _ => return Err(format!("unsupported sri arrangement: {}", arr_d)),
};
if shift > esize {
    return Err(format!("sri: shift {} out of range [1, {}]", shift, esize));
}
// immh_immb now in [esize, 2*esize-1], so immh != 0.
```

## Regression Property

Failing property: `sri_rejects_out_of_range_shift`

```rust
proptest! {
    #[test]
    #[ignore]
    fn sri_rejects_out_of_range_shift(
        rd in 0u32..=31,
        rn in 0u32..=31,
        arr in prop_oneof![Just("8b"),Just("16b"),Just("4h"),Just("8h"),
                           Just("2s"),Just("4s"),Just("2d")],
        extra in 1u32..=64u32,
    ) {
        let esize = match arr { "8b"|"16b"=>8, "4h"|"8h"=>16, "2s"|"4s"=>32, "2d"=>64, _=>0 };
        let shift_b = esize + 1 + (extra % esize); // in [esize+1, 2*esize]
        for &shift in &[0u32, shift_b] {
            let ops = vec![
                Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
                Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.to_string() },
                Operand::Imm(shift as i64),
            ];
            let res = encode_neon_sri(&ops);
            prop_assert!(res.is_err(),
                "sri {} shift {} is out of range [1, {}] but was accepted as {:?}",
                arr, shift, esize, res.ok());
        }
    }
}
```

**Minimal failing input:** `rd = 0, rn = 0, arr = "8b", extra = 1` →
`sri 8b shift 0 is out of range [1, 8] but was accepted as Some(Word(788546560))`.
Reproduce: `cargo test --lib neon_sri_pbt -- --ignored sri_rejects_out_of_range_shift`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/242
