# Bug Report: `encode_negs` silently accepts mixed W/X register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** Medium

## Summary

`encode_negs` derives the `sf` (operand-size) bit **only** from the destination register `Rd`, and ignores the width of the source `Rm`. ARMv8 requires all operands of `NEGS <Rd>, <Rm>` to share one width (`negs w0, x1` and `negs x0, w1` are illegal). `llvm-mc`/GAS reject them with "operand size mismatch", but this encoder returns `Ok(Word(...))` — assembling the instruction at the destination width with the (wrong-width) source register number.

This is the NEGS twin of the already-reported `encode_neg` width-mixing defect; `encode_negs` was not previously covered.

## Root Cause

```rust
pub(crate) fn encode_negs(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // width taken from Rd only
    let (rm, _) = get_reg(operands, 1)?;        // Rm width discarded (`_`)
    let sf = sf_bit(is_64);
    ...
}
```

The `_` discards `Rm`'s width, so no width-coherence check is possible.

## Reproduction

**Input:** `negs w0, x1`

**Expected:** `Err` — operand-size mismatch

**Actual:** `Ok(Word(0x6B2103E0))` — encodes a 32-bit `negs w0, <rm=1>` with `sf=0`

**Minimal failing input:** `encode_negs(&[Operand::Reg("w0".into()), Operand::Reg("x1".into())])`

Differential check: `echo 'negs w0, x1' | clang --target=aarch64-linux-gnu -c -x assembler - -o /tmp/a.o` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a mixed-width `negs` is assembled without diagnostic, producing an instruction whose source register width does not match the destination. Code that the reference assembler rejects slips through, with a wrong-semantics instruction in the output.

## Suggested Fix

After resolving `rm`, require its width to equal `is_64` (and likewise for the destination):

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rm, rm_is_64) = get_reg(operands, 1)?;
if rm_is_64 != is_64 {
    return Err("negs: all operands must have the same register width".into());
}
```

## Regression Property

Failing property: `negs_rejects_mixed_register_widths`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::negs_rejects_mixed_register_widths -- --ignored
prop_assert!(encode_negs(&[Operand::Reg("w0".into()), Operand::Reg("x1".into())]).is_err());
```

**GitHub Issue:** (none)
