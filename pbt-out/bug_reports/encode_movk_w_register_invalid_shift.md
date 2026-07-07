# Bug Report: `encode_movk` accepts invalid 32-bit W-register shifts

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movk`

## Summary

For 32-bit `W` destination registers, MOVK only permits `lsl #0` and `lsl #16`. `encode_movk` ignores register width when computing `hw`, so it accepts `lsl #32` and `lsl #48`, emitting invalid 32-bit MOVK encodings instead of returning `Err`.

## Reproduction

Failing property: `movk_w_reg_rejects_32_or_48_shift`

Minimal input:

```text
rd = 0, bad_amount = 32
```

`movk w0, #1, lsl #32` computes `hw = 2` and returns `Ok(Word(_))`, even though `hw=2` is only valid for 64-bit `X` destinations.

## Impact

The assembler emits architecturally invalid/undefined encodings for source that should be rejected.

## Suggested fix

Gate `hw = 2` and `hw = 3` on `is_64`:

```rust
match (*amount, is_64) {
    (0, _) => 0,
    (16, _) => 1,
    (32, true) => 2,
    (48, true) => 3,
    _ => return Err(format!("movk lsl shift {} invalid for {}-bit register", amount, if is_64 { 64 } else { 32 })),
}
```
