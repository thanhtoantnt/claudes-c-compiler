# Bug Report: `encode_tbz` accepts a test bit beyond the width of a 32-bit (W) destination

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_tbz`
**Severity:** High

## Summary

TBZ/TBNZ's bit field `b5:b40` is 6 bits, but the *architecturally valid* range
depends on the destination register width: **0..=63 for a 64-bit X register**,
and **0..=31 for a 32-bit W register** (a W register is only 32 bits wide, so
bits 32..=63 do not exist). The encoder discards `get_reg`'s `is_64` flag and
never restricts the bit by width, so `tbz w0, #40, t` is silently accepted and
assembled verbatim — testing a bit that does not exist in the register.

GAS and LLVM-MC reject this:
```
$ echo 'tbz w0, #40, t' | llvm-mc -triple=aarch64
error: immediate must be an integer in range [0, 31].
```

(This is the same root cause — discarded `is_64` — as the `encode_adrp`
32-bit-destination defect reported in
`encode_adrp_silent_32bit_destination.md`.)

## Root Cause

```rust
pub(crate) fn encode_tbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String> {
    let (rt, _) = get_reg(operands, 0)?;   // <-- is_64 discarded; width never checked
    let bit = get_imm(operands, 1)?;
    ...
    let b5 = ((bit as u32) >> 5) & 1;
    let b40 = (bit as u32) & 0x1F;          // no width-relative bound on `bit`
    ...
}
```

## Reproduction

**Input:** `tbz w0, #40, t`  →
`encode_tbz(&[Operand::Reg("w0"), Operand::Imm(40), Operand::Symbol("t")], false)`

**Expected:** `Err` — W is 32-bit; valid bit range is 0..=31.

**Actual:** `Ok(...)` encoding `b5:b40 = 40` (a bit beyond the register width).

**Minimal failing input:** `n = 0, bit = 40` (i.e. `tbz w0, #40, t`). Any bit in
`32..=63` against any W register triggers it.

## Impact

The assembled instruction tests a bit that does not exist in the 32-bit
destination. Architecturally the behaviour of such an encoding is
UNPREDICTABLE / the test can never be satisfied, so the branch behaves as an
unconditional pass-through — a silent correctness defect with no error from the
assembler. Same defect affects the TBNZ (`is_nz`) form.

## Suggested Fix

Track and use the register width, restricting the bit accordingly:
```rust
let (rt, is_64) = get_reg(operands, 0)?;
let bit = get_imm(operands, 1)?;
let max_bit = if is_64 { 63 } else { 31 };
if !(0..=max_bit).contains(&bit) {
    return Err(format!("tbz/tbnz: bit must be in 0..={}, got {}", max_bit, bit));
}
```

## Regression Property

Failing property: `prop_tbz_rejects_bit_beyond_w_width`
(in `src/backend/arm/assembler/encoder/prop_adrp_cbz_tbz.rs`, `#[ignore]`d so
the default `cargo test` stays green; reproduce with
`cargo test --lib -- --ignored prop_tbz_rejects_bit_beyond_w_width`).

```rust
#[test]
#[ignore = "documented bug: encode_tbz accepts bit 32..=63 on a 32-bit W destination (width not validated)"]
fn prop_tbz_rejects_bit_beyond_w_width(n in 0u32..=30u32, bit in 32u32..=63u32, is_nz in any::<bool>()) {
    let ops = vec![
        Operand::Reg(format!("w{}", n)),
        Operand::Imm(bit as i64),
        Operand::Symbol("t".into()),
    ];
    prop_assert!(encode_tbz(&ops, is_nz).is_err());
}
```

Minimal failing input today: `n = 0, bit = 40`.
