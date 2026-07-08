# Bug: `encode_negs` silently truncates shift amounts ≥ 64 (6-bit `imm6` field wrap)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_negs`
**Severity:** Medium (emits wrong-semantics machine code instead of erroring)

## Summary

`encode_negs` implements NEGS as `SUBS Rd, ZR, Rm {, <shift> #<amount>}`. The shift
amount is folded into the 6-bit `imm6` field with a bare `& 0x3F` mask and **never
validated** against the field width:

```rust
let word = (sf << 31) | (1 << 30) | (1 << 29) | (0b01011 << 24) | (shift_type << 22)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
```

`imm6` is a 6-bit field, so the maximum allocated value is `63`. Amounts `≥ 64` are
masked, changing the instruction's meaning silently instead of erroring.

## Minimal failing input

`negs x0, x1, lsl #64`  (64-bit destination, shift amount 64)

## Expected vs. actual

- **Expected:** `Err(...)` — `lsl #64` is out of range for a 64-bit register; a
  conforming assembler (llvm-mc / GAS) rejects it:
  `error: expected compatible register or logical immediate`.
- **Actual:** `Ok(Word(0xeb0003e0))` — `imm6 = 64 & 0x3F == 0`, so it silently
  becomes `negs x0, x1` (i.e. `lsl #0`).

Other inputs in the same class: `negs x0, x1, lsl #65` -> `Ok(Word(0xeb00083e0))`
(`imm6 = 1` -> silently becomes `lsl #1`).

## Impact

An out-of-range shift silently wraps, so a program requesting `lsl #64`/`#65`/...
assembles to a *different* instruction with no diagnostic. The same `& 0x3F` masking
without validation is also present in sibling functions (`encode_neg`, `encode_mvn`,
`encode_logical`, ...), so this is a class-wide pattern.

## Suggested fix

Validate the shift amount against the field/register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for {}-bit register",
                       shift_amount, if is_64 { 64 } else { 32 }));
}
```

(The validation guarantees the value fits, so the `& 0x3F` mask can then be dropped.)

## Reproduction

Property test `negs_rejects_64bit_shift_amount_above_field` in the `negs_props`
module of `src/backend/arm/assembler/encoder/data_processing.rs`.

Minimal input: `rd = 0, rm = 0, sk = 0, amount = 64`.

Run with: `cargo test --lib negs_props::negs_rejects_64bit_shift_amount_above_field`
