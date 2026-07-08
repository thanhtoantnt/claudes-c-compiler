# Bug — `encode_add_sub` silently truncates 64-bit shifted-register LSL (`imm6`)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_add_sub` (shifted-register branch)
**Severity:** high — produces a *valid-but-wrong* encoding with no diagnostic.

## Where
```rust
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
    let st = match kind.as_str() { "lsl" => 0b00u32, "lsr" => 0b01, "asr" => 0b10, _ => 0b00 };
    (st, *amount)
} else {
    (0, 0)
};
let word = ... | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

## Problem
For the *Add (shifted register)* form, `imm6` (bits 15:10) is a 6-bit field. For
`LSL` on a 64-bit (X) register the architecturally valid range is `0..=63`; `LSL #64`
and above are UNDEFINED for `LSL` (only `LSR`/`ASR` permit `#64`, and only via a
special encoding). The current code masks with `& 0x3F`, so `add x0, x1, x2, lsl #64`
silently becomes `lsl #0` and `lsl #65` becomes `lsl #1` — a different instruction,
no diagnostic. GAS rejects `lsl #64` on general registers.

## Reproducing property (fails)
`tests::xreg_lsl_shift_above_63_must_be_rejected`
```text
minimal failing input: rd = 0, rn = 0, rm = 0, amount = 64
assertion failed: encode_add_sub(&ops, false, false).is_err()
```

## Related: the 32-bit (W) case is also unvalidated
Existing test #11 (`w_reg_shifted_form_rejects_shift_above_31`) asserts the W-register
case must be rejected for `amount in 32..=63`. The encoder currently masks that path
with the same `& 0x3F` and does **not** return `Err`, so test #11 would fail too if
run in isolation — both the W and X cases of the shifted-register form need range
validation.

## Suggested fix
Reject out-of-range shift amounts instead of masking:
```rust
let cap = match kind {
    "lsr" | "asr" if is_64 => 64,
    "lsr" | "asr"           => 32,
    _ if is_64              => 63,   // lsl, 64-bit
    _                       => 31,   // lsl, 32-bit
};
if shift_amount > cap {
    return Err(format!("shift #{} out of range for {}-bit register",
                       shift_amount, if is_64 { 64 } else { 32 }));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/2
