# Bug Report: `encode_umaddl` accepts X source registers

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_umaddl`

## Summary

`encode_umaddl` discards the width flags returned by `get_reg` for the multiplicand source registers. UMADDL requires 32-bit source operands (`Wn`, `Wm`) and 64-bit destination/accumulator operands (`Xd`, `Xa`), but the encoder accepts 64-bit X source registers and still emits an instruction word.

## Reproduction

Failing property: `umaddl_rejects_x_source_registers`

Minimal input:

```text
umaddl x0, x1, x2, x3
```

Actual behavior: returns `Ok(Word(_))` instead of `Err`.

## Impact

Invalid UMADDL source is accepted and assembled into an architecturally invalid/undefined instruction form instead of being diagnosed.

## Suggested fix

Require the multiplicand operands to be W registers:

```rust
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if rn64 || rm64 {
    return Err("umaddl requires W source registers".to_string());
}
```
## Regression Property

Failing property: `umaddl_rejects_x_source_registers`

```rust
prop_assert!(encode_umaddl(&[xreg(0), xreg(1), xreg(2), xreg(3)]).is_err());  // X sources
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/200
