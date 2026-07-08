# Bug Report: `encode_movn` silently normalizes invalid shift amounts

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movn`

## Summary

`encode_movn` computes the MOVN halfword selector as `amount / 16` for `lsl` shifts. It does not require the amount to be an exact valid MOVN shift, so non-multiple-of-16 values are silently floored to a different shift.

## Reproduction

Failing property: `movn_rejects_non_multiple_of_16_shift`

Minimal input:

```text
rd = 0, amount = 1
```

`movn x0, #1, lsl #1` encodes `hw = 1 / 16 = 0`, i.e. no shift, instead of returning `Err`. `lsl #17` similarly encodes as `lsl #16`.

## Impact

Invalid assembly is accepted and encoded as a different instruction than the programmer wrote.

## Suggested fix

Reject non-`lsl` shifts and require exact valid amounts:

```rust
match (*amount, is_64) {
    (0, _) => 0,
    (16, _) => 1,
    (32, true) => 2,
    (48, true) => 3,
    _ => return Err(format!("movn invalid lsl shift: {}", amount)),
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/63
