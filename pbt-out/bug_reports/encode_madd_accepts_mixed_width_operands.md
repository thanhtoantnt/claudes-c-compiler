# Bug: `encode_madd` does not reject mixed-width operands

- **Target:** `src/backend/arm/assembler/encoder/data_processing.rs` — `fn encode_madd`
- **Severity:** Low–Medium (operand-width / `sf` mismatch, no diagnostic)
- **Witness (property, FAILS):** `madd_props::madd_rejects_mixed_width_operands`
  (pre-existing in-tree test; re-confirmed during this campaign)

## Summary
The ARMv8 ARM requires all four operands of `MADD <Rd>,<Rn>,<Rm>,<Ra>` to share
one register width (all `X` or all `W`). `encode_madd` derives `sf` **only** from
operand 0 (`Rd`) and ignores the width of `Rn`, `Rm`, `Ra`. A mixed-width form
such as `madd x0, w1, x2, x3` is therefore accepted and encoded as a 64-bit
operation (`sf=1`) that reads `Rn` as if it were `X1`.

## Reproduction
Property `madd_rejects_mixed_width_operands` fails on minimal input:

```
madd x0, w1, x2, x3   ->   accepted; sf = 1 (taken from Rd), Rn's W width ignored
```

## Root cause
```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // width discarded
let (rm, _) = get_reg(operands, 2)?;   // width discarded
let (ra, _) = get_reg(operands, 3)?;   // width discarded
let sf = sf_bit(is_64);
```

GNU `as` and LLVM reject mixed-width `MADD` operands.

## Expected behavior
Reject with an error when the four operand widths are not all identical.

## Fix sketch
Collect the `is_64` flag from all four `get_reg` calls and assert equality:
`if rn_is64 != is_64 || rm_is64 != is_64 || ra_is64 != is_64 { return Err(...); }`

## Related
`madd_props::madd_rejects_sp_operand` (separate bug report) — same missing
validation layer in `encode_madd`.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/56
