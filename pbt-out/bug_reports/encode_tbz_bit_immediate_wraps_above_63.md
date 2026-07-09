# Bug Report: `encode_tbz` silently wraps the test-bit immediate (field is 6-bit unsigned)

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_tbz`
**Severity:** High

## Summary

TBZ/TBNZ's bit selector is the 6-bit field `b5:b40` (valid **0..=63**; for a
32-bit W register the architecture further restricts it to 0..=31). The encoder
computes the field by masking:

```rust
let b5  = ((bit as u32) >> 5) & 1;
let b40 = (bit as u32) & 0x1F;
```

with **no range validation**. Any value silently wraps into 0..=63:

- values `>= 64` wrap by masking (`& 0x1F`, `>> 5 & 1`);
  e.g. `tbz x0, #64, t` becomes `tbz x0, #0, t`.
- negative values wrap via the `i64 as u32` cast then masking;
  e.g. `tbz x0, #-1, t` becomes `tbz x0, #63, t`.

Per the ARMv8-A ARM (TBZ/TBNZ) and GAS/LLVM-MC, `tbz x0, #64, t` and
`tbz x0, #-1, t` must be rejected ("immediate must be an integer in range
[0, 63]" / "immediate value out of range").

## Root Cause

```rust
pub(crate) fn encode_tbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String> {
    let (rt, _) = get_reg(operands, 0)?;
    let bit = get_imm(operands, 1)?;
    ...
    let b5 = ((bit as u32) >> 5) & 1;     // no range check on `bit`
    let b40 = (bit as u32) & 0x1F;        // masking wraps out-of-range values
    ...
}
```

(Note: a sibling report `encode_tbz-silent-truncation-of-bit-immediate.md`
claims the valid range is 0..=31; that is incorrect for the 64-bit form. The
architectural encoding field is 6-bit, valid 0..=63 for an X register.)

## Reproduction

**Input:** `tbz x0, #64, t`  →  `encode_tbz(&[Operand::Reg("x0"), Operand::Imm(64), Operand::Symbol("t")], false)`

**Expected:** `Err` (bit must be in 0..=63).

**Actual:** `Ok(...)` encoding `b5:b40 = 0` (i.e. `tbz x0, #0, t`).

Likewise `tbz x0, #-1, t` → `Ok(...)` encoding bit `63`.

**Minimal failing input:** `bit = 64` (and `bit = -1` for the negative case).

## Impact

Silent truncation/wrapping: the user requests a test of one bit but the assembled
instruction tests a *different* bit, with no error. For LTO'd / generated code
this produces an instruction that branches on the wrong bit, a correctness bug
that is extremely hard to trace back to the assembler. Same defect affects the
`is_nz` (TBNZ) form.

## Suggested Fix

Validate the immediate before splitting it into `b5`/`b40` (and, for a fully
spec-compliant encoder, additionally restrict to 0..=31 when the destination is
a 32-bit W register):

```rust
let bit = get_imm(operands, 1)?;
if !(0..=63).contains(&bit) {
    return Err(format!("tbz/tbnz: bit must be in 0..=63, got {}", bit));
}
```

## Regression Property

Failing properties: `prop_tbz_rejects_bit_above_63` and `prop_tbz_rejects_negative_bit`
(in `src/backend/arm/assembler/encoder/prop_adrp_cbz_tbz.rs`, both `#[ignore]`d so
the default `cargo test` stays green; reproduce with
`cargo test --lib -- --ignored prop_tbz_rejects_bit_above_63 prop_tbz_rejects_negative_bit`).

```rust
#[test]
#[ignore = "documented bug: encode_tbz silently truncates bit >= 64 into 0..=63 (field is 6-bit unsigned)"]
fn prop_tbz_rejects_bit_above_63((rt_name, _) in arb_gp_reg(), bit in 64u32..=4096u32, is_nz in any::<bool>()) {
    let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit as i64), Operand::Symbol("t".into())];
    prop_assert!(encode_tbz(&ops, is_nz).is_err());
}

#[test]
#[ignore = "documented bug: encode_tbz silently wraps a negative bit via `i64 as u32` then masking (field is unsigned)"]
fn prop_tbz_rejects_negative_bit((rt_name, _) in arb_gp_reg(), bit in -4096i64..=-1i64, is_nz in any::<bool>()) {
    let ops = vec![Operand::Reg(rt_name), Operand::Imm(bit), Operand::Symbol("t".into())];
    prop_assert!(encode_tbz(&ops, is_nz).is_err());
}
```

Minimal failing inputs today: `bit = 64` and `bit = -1`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/304
