# Bug Report: `encode_orn`/`encode_bic`/`encode_bics`/`encode_mvn` coerce unknown shift kinds to LSL

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_orn`, `encode_bic`, `encode_bics`, `encode_mvn`
**Severity:** Medium

## Summary

Only the four shift kinds `LSL`, `LSR`, `ASR`, `ROR` are defined for the AArch64
*Logical (shifted register)* class (ARMv8 ARM §C4.1.115). An unrecognized shift kind
(such as a typo `foo`, a case variant `LSL`, or a mnemonic from another instruction
class like `extend`/`msl`) has no encoding and must be rejected. Instead,
`encode_orn`, `encode_bic`, `encode_bics`, and `encode_mvn` each contain a catch-all
match arm `_ => 0b00` that silently maps **any** unrecognized kind to `LSL` and
returns `Ok`. The shift *amount* is then placed verbatim, so the assembled
instruction is a valid LSL variant — differing from the source text with no
diagnostic.

This is the **same defect class** already reported for `encode_eon`
(`encode_eon_unknown_shift_kind_maps_to_lsl.md`); this report covers the **four
remaining** logical-NOT encoders that were not previously reported. (The existing
`encode_mvn_silent_undefined_w_reg_shift.md` report is a muddled description of a
*non-existent* W-register ROR/ASR problem — ROR/ASR have explicit arms here; the
genuine defect is this catch-all coercion to LSL for *any* unknown kind.)

## Root Cause

Each encoder resolves the shift kind with a `match` whose final arm coerces the
unknown case to LSL instead of erroring:

```rust
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(N) {
    let st = match kind.as_str() {
        "lsl" => 0b00u32,
        "lsr" => 0b01,
        "asr" => 0b10,
        "ror" => 0b11,
        _ => 0b00,          // <-- BUG: silently treats unknown kind as LSL
    };
    (st, *amount)
} else {
    (0, 0)
};
```

## Reproduction

| Source text               | Expected | Actual (`Ok`, identical to `lsl #0`) |
|---------------------------|----------|--------------------------------------|
| `orn x0, x1, x2, foo #0`  | `Err`    | `Ok(0xAA220020)`  (== `orn x0,x1,x2,lsl #0`) |
| `bic x0, x1, x2, foo #0`  | `Err`    | `Ok(0x8A220020)`  (== `bic x0,x1,x2,lsl #0`) |
| `bics x0, x1, x2, foo #0` | `Err`    | `Ok(0xEA220020)`  (== `bics x0,x1,x2,lsl #0`) |
| `mvn x0, x1, foo #0`      | `Err`    | `Ok(0xAA2103E0)`  (== `mvn x0,x1,lsl #0`) |

The `shift_type` field (bits 23:22) of the encoded word is `0b00` in every case,
proving the coercion to LSL; the supplied amount is otherwise preserved.

## Impact

A typo in the shift mnemonic (e.g. `foo`, an upper-case `LSL`, or `msl`/`extend`
belonging to other instruction classes) assembles into a valid-but-unintended LSL
instruction. Because the encoding is valid, the mistake produces no diagnostic and
silently changes the operation. Compiler/assembler front-ends that pass an
unrecognized shift token through get an LSL instead of an error, masking upstream
parser bugs.

## Suggested Fix

Replace the catch-all with an error so unknown kinds are rejected:

```rust
let st = match kind.as_str() {
    "lsl" => 0b00u32,
    "lsr" => 0b01,
    "asr" => 0b10,
    "ror" => 0b11,
    other => return Err(format!("unsupported shift kind: {}", other)),
};
```

(Apply uniformly to `encode_orn`, `encode_bic`, `encode_bics`, `encode_mvn` — and
their already-fixed sibling `encode_eon` for consistency.)

## Regression Property

Failing properties: `orn_rejects_unknown_shift_kind`,
`bic_rejects_unknown_shift_kind`, `bics_rejects_unknown_shift_kind`,
`mvn_rejects_unknown_shift_kind` (in `data_processing_logical_not_pbt.rs`, marked
`#[ignore]`).

```rust
prop_assert!(encode_orn(&[xreg(0), xreg(1), xreg(2), shift("foo", 0)]).is_err());
prop_assert!(encode_bic(&[xreg(0), xreg(1), xreg(2), shift("foo", 0)]).is_err());
prop_assert!(encode_bics(&[xreg(0), xreg(1), xreg(2), shift("foo", 0)]).is_err());
prop_assert!(encode_mvn(&[xreg(0), xreg(1), shift("foo", 0)]).is_err());
```
