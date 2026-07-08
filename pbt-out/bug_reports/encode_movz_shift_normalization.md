# Bug Report: `encode_movz` silently normalizes invalid shift amounts

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movz`

## Summary

`encode_movz` computes the MOVZ halfword selector as `amount / 16` for `lsl` shifts. It does not require the shift amount to be one of the valid MOVZ amounts (`0`, `16`, `32`, `48` for 64-bit registers; `0`, `16` for 32-bit registers). Non-multiple-of-16 shifts are silently floored to a different valid shift.

## Reproduction

Failing property: `movz_rejects_non_multiple_of_16_shift`

Minimal input:

```text
rd = 0, hw = 0, rem = 1   (amount = 1)
```

`movz x0, #1, lsl #1` encodes `hw = 1 / 16 = 0`, producing the same word as `movz x0, #1` instead of returning `Err`.

Relevant source:

```rust
if kind == "lsl" {
    *amount / 16
} else {
    0
}
```

A related variant is that non-`lsl` shift kinds (`lsr`, `asr`, `ror`) are silently treated as `hw = 0` rather than rejected, even though MOVZ supports only `LSL`.

## Impact

Silent miscompilation: invalid shifts assemble to the wrong constant without any diagnostic.

## Suggested fix

Require `kind == "lsl"` and match only the exact valid shift amounts:

```rust
let hw = match *amount {
    0 => 0,
    16 => 1,
    32 if is_64 => 2,
    48 if is_64 => 3,
    _ => return Err(format!("movz invalid lsl shift: {}", amount)),
};
```

Apply the same fix to `encode_movk` and `encode_movn`, which duplicate the same shift-normalization pattern.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/66
