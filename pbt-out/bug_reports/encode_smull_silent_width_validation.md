# Bug Report: `encode_smull` silently accepts invalid operand widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_smull`

## Summary

`encode_smull` discards the width flags returned by `get_reg` for all three operands and hardcodes `sf = 1`. The mnemonic `SMULL` requires a 64-bit destination (`Xd`) and 32-bit sources (`Wn`, `Wm`). Instead, the encoder accepts invalid widths and silently assembles them as the 64-bit `SMADDL` alias form.

## Reproduction

Failing property: `smull_rejects_wrong_width_destination`

Minimal examples observed during the run:

```text
smull w0, w1, w2
smull x0, x1, x2
```

Actual behavior: both return `Ok(Word(_))` instead of `Err`.

## Impact

Invalid `SMULL` source is accepted and encoded as a different width form than written. That hides operand-size mistakes and can make the produced machine code read a different architectural view of the registers than the assembly text indicates.

## Suggested fix

Check all three `is_64` flags returned by `get_reg` and reject mismatches:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
let (rm, rm_is_64) = get_reg(operands, 2)?;
if !rd_is_64 || rn_is_64 || rm_is_64 {
    return Err("smull requires Xd, Wn, Wm".into());
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/97
