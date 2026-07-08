# Bug: `encode_negs` silently encodes UNALLOCATED shift amounts for 32-bit (W) registers

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_negs`
**Severity:** Medium (emits unallocated machine code instead of erroring)

## Summary

`encode_negs` implements NEGS as `SUBS Rd, ZR, Rm {, <shift> #<amount>}`. The shift
amount is folded into the 6-bit `imm6` field with a bare `& 0x3F` mask and **never
validated** against the destination register width:

```rust
let word = (sf << 31) | (1 << 30) | (1 << 29) | (0b01011 << 24) | (shift_type << 22)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
```

Per the ARMv8 ARM, for the 32-bit (`sf=0`) SUBS (shifted register) encoding the
`imm6` field is allocated only in `0..=31`; values `32..=63` are **UNALLOCATED**.
`encode_negs` does not check `sf`, so these assemble silently to unallocated words.

## Minimal failing input

`negs w0, w1, lsl #32`  (32-bit destination, shift amount 32)

## Expected vs. actual

- **Expected:** `Err(...)` — the encoding is unallocated; a conforming assembler
  (llvm-mc / GAS) rejects it: `error: expected compatible register or logical immediate`.
- **Actual:** `Ok(Word(0x6b0083e0))` — `imm6 = 32`, an UNALLOCATED encoding.

Other inputs in the same class: `negs w0, w1, lsl #40` -> `Ok(Word(0x6b00a3e0))` (`imm6 = 40`).

## Impact

An invalid program assembles to a broken object without any diagnostic. The same
`& 0x3F` masking without width validation is also present in sibling functions
(`encode_neg`, `encode_mvn`, `encode_logical`, ...), so this is a class-wide pattern.

## Suggested fix

After resolving `is_64`, validate the shift amount against the width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for {}-bit register",
                       shift_amount, if is_64 { 64 } else { 32 }));
}
```

## Reproduction

Property test `negs_rejects_32bit_unallocated_shift_amount` in the `negs_props`
module of `src/backend/arm/assembler/encoder/data_processing.rs`.

Minimal input: `rd = 0, rm = 0, sk = 0, amount = 32`.

Run with: `cargo test --lib negs_props::negs_rejects_32bit_unallocated_shift_amount`
