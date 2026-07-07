# Bug Report: `encode_movz` accepts invalid 32-bit W-register shifts

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movz`

## Summary

`encode_movz` ignores the destination register width when validating the MOVZ halfword selector. For 32-bit `W` registers, only `lsl #0` and `lsl #16` are valid. The encoder accepts `lsl #32` and `lsl #48`, emitting architecturally UNDEFINED encodings (`hw = 2` / `hw = 3`) instead of returning `Err`.

## Reproduction

Failing property: `movz_w_reg_rejects_32_or_48_shift`

Minimal input:

```text
rd = 0, bad_amount = 32
```

`movz w0, #1, lsl #32` computes `hw = 32 / 16 = 2` and encodes that value. The `hw` computation never checks `is_64`.

Relevant source:

```rust
let rd_name = get_reg_name(operands, 0)?;
let is_64 = rd_name.starts_with('x');
...
if kind == "lsl" {
    *amount / 16
}
```

## Impact

The assembler emits UNDEFINED encodings for invalid 32-bit MOVZ forms, rather than rejecting source code that a reference AArch64 assembler would diagnose.

## Suggested fix

Gate `hw = 2` and `hw = 3` on `is_64`:

```rust
match (*amount, is_64) {
    (0, _) => 0,
    (16, _) => 1,
    (32, true) => 2,
    (48, true) => 3,
    _ => return Err(format!("movz lsl shift {} invalid for {}-bit register", amount, if is_64 { 64 } else { 32 })),
}
```

Apply the same width gate to `encode_movk` and `encode_movn`.
