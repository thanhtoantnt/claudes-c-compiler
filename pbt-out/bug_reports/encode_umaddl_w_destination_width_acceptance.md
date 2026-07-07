# Bug Report: `encode_umaddl` accepts W destination register

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_umaddl`

## Summary

`encode_umaddl` discards the width flag returned by `get_reg` for the destination register. UMADDL requires a 64-bit destination (`Xd`), but the encoder accepts a 32-bit `Wd` destination and still emits the `sf=1` long multiply-add encoding.

## Reproduction

Failing property: `umaddl_rejects_w_destination_register`

Minimal input:

```text
umaddl w0, w0, w0, x0
```

Actual behavior: returns `Ok(Word(_))` instead of `Err`.

## Impact

Invalid UMADDL source is accepted and assembled into an architecturally invalid/undefined instruction form instead of being diagnosed.

## Suggested fix

Require the destination and accumulator operands to be 64-bit:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (ra, ra64) = get_reg(operands, 3)?;
if !rd64 || !ra64 {
    return Err("umaddl requires X destination and X accumulator".to_string());
}
```
