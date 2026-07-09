# Bug — `encode_neon_shift_imm` emits UNALLOCATED encoding for out-of-range shift

**File:** `src/backend/arm/assembler/encoder/neon.rs` (line ~373)
**Function:** `pub(crate) fn encode_neon_shift_imm(operands: &[Operand], _is_unsigned: bool) -> Result<EncodeResult, String>`
**Witness:** `prop_out_of_range_shifts_must_be_rejected`
(`src/backend/arm/assembler/encoder/neon_shift_imm_pbt.rs:120`)

## Summary

The encoder performs **no range validation** on the `shift` immediate. It masks
the computed `immh:immb` field instead of bounding the shift, so an invalid
shift is accepted and the emitted word carries `immh == 0`, which the ARMv8
ARM defines as **UNALLOCATED** for this instruction group.

## Architecture constraint

Per the ARMv8 ARM ("Advanced SIMD shift by immediate"), USHR/SSHR require
`1 <= shift <= esize` and `immh != 0`. `immh == 0` is reserved / UNALLOCATED.

## Code at fault

```rust
"8b" | "16b" => (8u32, (16 - shift as u32) & 0xF),   // mask only — no bounds check
"4h" | "8h"  => (16,   (32 - shift as u32) & 0x1F),
"2s" | "4s"  => (32,   (64 - shift as u32) & 0x3F),
"2d"         => (64,   (128 - shift as u32) & 0x7F),
```

There is no guard rejecting shifts outside `[1, esize]`.

## Minimal input

```text
operands = [ Vd.8b, Vn.8b, #0 ]
```

`immh:immb = (16 - 0) & 0xF = 0` ⇒ top nibble `immh == 0000` ⇒ UNALLOCATED.

## Expected vs. actual

- **Expected:** `Err(...)` — a diagnostic rejecting `shift == 0`.
- **Actual:** `Ok(EncodeResult::Word(0x2F000020))`, an instruction the hardware
  treats as undefined, emitted with no error.

## Impact

The assembler silently produces an UNALLOCATED instruction for an out-of-range
shift amount. The same masking also lets over-large or negative shifts wrap to
an unrelated `esize`/`shift` pair (silent re-encoding). A sibling encoder,
`encode_neon_sqshrun`, performs exactly this check
(`if shift == 0 || shift > element_bits { return Err(...) }`), confirming the
omission is a regression rather than an intentional design choice.

## Witness

`prop_out_of_range_shifts_must_be_rejected`
(`src/backend/arm/assembler/encoder/neon_shift_imm_pbt.rs:120`) asserts that
`shift == 0` returns `Err` for every arrangement. Fails on current code (the
encoder returns `Ok`). Marked `#[ignore]` so the default `cargo test` stays
green; run it explicitly to reproduce.

```
minimal failing input: rd = 0, rn = 0
shift=0 must be rejected for 8b (immh would be 0)
```

- counterexample (shrunk): `rd = 0, rn = 0` → `shift=0` accepted, `immh == 0`
- reproduce: `cargo test --lib neon_shift_imm_pbt::prop_out_of_range_shifts_must_be_rejected -- --ignored`

## Fix

Range-check before masking (mirror `encode_neon_sqshrun`):

```rust
if shift < 1 || shift > elem_bits as i64 {
    return Err(format!("shift {} out of range for {}-bit elements", shift, elem_bits));
}
```
