# Bug Report — `encode_neg` silently truncates shift amount / rewrites invalid shift kind

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_neg`
**Status:** Confirmed by failing property `neg_x_reg_rejects_shift_above_63`. (The sibling
properties `neg_w_reg_rejects_shift_above_31` and `neg_rejects_ror_shift`, already present
in the suite, fail for the same root cause.)

## Summary

`encode_neg` accepts a shift operand and writes the amount straight into the imm6 field
after masking with `& 0x3F`, and maps any unrecognized shift kind to `lsl` via the default
arm of a `match`. No range or kind validation is performed, so out-of-range and
architecturally-undefined inputs are silently rewritten into a *valid-looking* encoding
rather than rejected:

```rust
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
    let st = match kind.as_str() {
        "lsl" => 0b00u32,
        "lsr" => 0b01,
        "asr" => 0b10,
        _ => 0b00,          // ← ROR (reserved for ADD/SUB) silently becomes LSL
    };
    (st, *amount)            // ← amount not range-checked
} else { (0, 0) };
let word = ... | ((shift_amount & 0x3F) << 10) | ...;   // ← 64+ silently truncated
```

Per the ARMv8 ARM (C4.1.4 / C4.1.66): for the add/sub shifted-register form the imm6 shift
amount must be `0..=63` when `sf=1` (64-bit) and `0..=31` when `sf=0` (32-bit), and only
LSL/LSR/ASR are permitted (ROR is reserved for the logical class). Out-of-range amounts and
ROR are UNDEFINED and must be diagnosed.

## Reproduction

Failing property: `neg_x_reg_rejects_shift_above_63`

Minimal examples:

```text
neg x0, x1, lsl #64      # actual: Ok, imm6 = (64 & 0x3F) = 0  → encodes as "neg x0, x1"
neg x0, x1, lsl #100     # actual: Ok, imm6 = 36               → encodes as "neg x0, x1, lsl #36"
neg x0, x1, lsr #40      # actual: Ok, imm6 = 40               → encodes "neg x0, x1, lsr #40"
neg x0, x1, ror #5       # actual: Ok, shift_type = LSL        → encodes "neg x0, x1, lsl #5"
```

Expected behavior: `Err` for every one of the above.

## Impact

A shift amount typo, a macro/expander bug, or an upstream parser that doesn't pre-validate
yields an instruction that assembles *successfully* but performs a different shift — or no
shift at all. This is a silent miscompilation: the produced machine code differs from the
assembly source with no diagnostic. `encode_negs`, which shares this exact code shape, has
the identical defect.

## Suggested fix

Validate before encoding:

```rust
let (shift_type, shift_amount) = match operands.get(2) {
    Some(Operand::Shift { kind, amount }) => {
        let st = match kind.as_str() {
            "lsl" => 0b00u32,
            "lsr" => 0b01,
            "asr" => 0b10,
            other => return Err(format!("neg does not support shift kind '{}'", other)),
        };
        let max = if is_64 { 63 } else { 31 };
        if *amount > max {
            return Err(format!("neg shift amount {} out of range 0..={}", amount, max));
        }
        (st, *amount)
    }
    _ => (0, 0),
};
```

This makes the already-failing negative-contract properties `neg_w_reg_rejects_shift_above_31`,
`neg_rejects_ror_shift`, and `neg_x_reg_rejects_shift_above_63` pass.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/77
