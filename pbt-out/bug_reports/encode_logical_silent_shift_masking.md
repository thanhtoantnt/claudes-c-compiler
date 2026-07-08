# Bug Report — `encode_logical` silently masks out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_logical(operands, opc)`
**Scalar shifted-register branch** (AND / ORR / EOR / ANDS `Xd, Xn, Xm, <shift> #imm`).

## Summary

`encode_logical` accepts **any** shift amount and silently truncates it into the
6-bit `imm6` field with `& 0x3F`, instead of validating it against the
architecture's legal range. This produces a **valid-looking but architecturally
UNDEFINED** encoding instead of an assembly error. The bug is symmetric across
register widths but only the 32-bit (W) half was previously covered by a test;
this campaign added the missing 64-bit (X) negative contract, which also fails.

## Root cause

In the shifted-register path:

```rust
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
    let st = match kind.as_str() { "lsl"=>0, "lsr"=>1, "asr"=>2, "ror"=>3, _=>0 };
    (st, *amount)                       // <-- amount never range-checked
} else { (0, 0) };

let word = ... | ((shift_amount & 0x3F) << 10) | ...;   // <-- silent mask
```

Per the ARMv8 ARM, the shifted-register `imm6` field is:
- **0..=31** for 32-bit (`sf == 0`) registers,
- **0..=63** for 64-bit (`sf == 1`) registers.

Values outside those ranges are UNDEFINED and real assemblers (GAS, `llvm-mc`)
reject them. Here they are accepted and wrapped.

## Reproduction (property-based, minimal cases)

Two properties fail with identical root cause. Both currently FAIL.

| Property | Minimal failing input | Why it's wrong |
|---|---|---|
| `logical_w_reg_rejects_shift_above_31` (pre-existing) | `and w0, w0, w0, lsl #32` | `#32` masked into 6 bits → emits `lsl #32` for a W reg (UNDEFINED) |
| `logical_x_reg_rejects_shift_above_63` (**new**) | `and x0, x0, x0, lsl #64` | `#64 & 0x3F == 0` → emits `lsl #0` (silently different instruction!) |

The X-register case is the more dangerous variant: `lsl #64` **masquerades as
`lsl #0`** (64 mod 64 == 0), so the assembled instruction computes a completely
different value than the source text describes, with no diagnostic.

Run:
```
cargo test --lib logical_x_reg_rejects_shift_above_63   # FAILS
cargo test --lib logical_w_reg_rejects_shift_above_31   # FAILS (pre-existing)
```

## Impact

- Wrong code generation with no error: an instruction the user wrote to shift
  by `N` (N >= 64, or N >= 32 for W regs) is assembled as a shift by `N mod 64`,
  which is almost never what was intended.
- Violates the principle (matched by the rest of the encoder, e.g.
  `encode_add_sub`'s `extend_amount_above_7_must_be_rejected`) that
  unrepresentable operand values are rejected with `Err`, not coerced.

## Suggested fix

Validate `amount` against the width before placing it, mirroring the existing
immediate-form rejection of non-bitmask values:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for logical (max {})",
                       shift_amount, max_shift));
}
let word = ... | ((shift_amount & 0x3F) << 10) | ...;
```

The same defect exists in the sibling `encode_orn` / `encode_eon` / `encode_bic` /
`encode_bics` shifted-register paths (identical `& 0x3F` masking with no width
check) and should be fixed there too.

## Properties covering `encode_logical` (status)

| # | Property | Status |
|---|---|---|
| 1 | `logical_register_form_field_placement` | pass |
| 2 | `logical_register_form_shift_mapping` (0..=63, valid range) | pass |
| 3 | `logical_sf_tracks_width` | pass |
| 4 | `logical_w_reg_rejects_shift_above_31` (negative) | FAIL — this bug |
| 4b| `logical_x_reg_rejects_shift_above_63` (negative, **new**) | FAIL — this bug |
| 5 | `logical_immediate_form_roundtrips` (independent ARM-ARM decoder oracle) | pass |
| 6 | `logical_immediate_rejects_non_bitmask` (0 / all-ones -> Err) | pass |

## Regression property

Failing properties:
- `logical_w_reg_rejects_shift_above_31`
- `logical_x_reg_rejects_shift_above_63`

```rust
prop_assert!(encode_logical(&[wreg(0), wreg(1), wreg(2)], "and", "lsl", 32).is_err());
prop_assert!(encode_logical(&[xreg(0), xreg(1), xreg(2)], "and", "lsl", 64).is_err());
```
