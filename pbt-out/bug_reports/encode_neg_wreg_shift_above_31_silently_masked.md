# Bug — `encode_neg` silently masks out-of-range shift amounts for W registers

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_neg` (NEG = alias of `SUB Rd, XZR, Rm [, shift]`)
**Found by:** property-based test `data_processing::tests::neg_w_reg_rejects_shift_above_31`

## Defect

`encode_neg` emits a machine word for shift amounts `32..=63` on 32-bit (W)
registers instead of rejecting them.

**Spec:** ARMv8 ARM §C4.1.4 / §C4.1.66 — for the add/sub shifted-register form
with `sf=0` (32-bit), the imm6 shift amount **must** be in `0..=31`. Amounts
`32..=63` are UNDEFINED.

## Code

```rust
let word = (sf << 31) | (1 << 30) | (0b01011 << 24) | (shift_type << 22)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
//                       ^^^^^^^^^^^^^^^^^  no range check; 32..=63 silently accepted for W
```

## Reproduction

`cargo test --lib data_processing::tests::neg_w_reg_rejects_shift_above_31`

```
minimal failing input: rd = 0, rm = 0, amount = 32, sk = 0   (neg w0, w0, lsl #32)
assertion failed: encode_neg(&ops).is_err()
```

`neg w0, w0, lsl #32` returns `Ok` with `imm6 = 32`. Expected: `Err`, as GAS/LLVM do.

## Impact

Silent codegen corruption with no diagnostic: a `neg` with an out-of-range W-reg
shift produces a word whose runtime semantics are UNDEFINED by the architecture.

## Suggested fix

Range-check the shift against the register width before encoding:

```rust
let max = if is_64 { 63 } else { 31 };
if shift_amount > max {
    return Err(format!("neg shift amount {} out of range 0..={}", shift_amount, max));
}
```
