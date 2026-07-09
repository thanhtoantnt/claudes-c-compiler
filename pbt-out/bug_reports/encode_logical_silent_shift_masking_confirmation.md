# Bug Report: `encode_logical` silently masks/truncates out-of-range shift amounts

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_logical`
**Severity:** High (emits architecturally UNDEFINED / silently-different instructions)

## Summary

The scalar shifted-register branch of `encode_logical` (AND / ORR / EOR / ANDS
`Rd, Rn, Rm, <shift> #imm`) masks the shift amount into the 6-bit `imm6` field
with `& 0x3F` and **never validates it against the register width**. Per the
ARMv8 ARM ("Logical (shifted register)"), `imm6` is legal only for:

- **0..=31** when `sf == 0` (32-bit `W` registers) — values `32..=63` are
  UNDEFINED / CONSTRAINED UNPREDICTABLE, and are *silently accepted* here;
- **0..=63** when `sf == 1` (64-bit `X` registers) — values `>= 64` are
  silently **truncated** (`lsl #64` → `lsl #0`, `lsl #70` → `lsl #6`), i.e.
  the emitted instruction computes a different value than the source text.

This is a **confirmation** of the bug already filed in this repository as
`encode_logical_silent_shift_masking.md` (GitHub issue #54) and
`encode_logical_undefined_w_register_shift.md` (GitHub issue #55). The new
evidence here is an **authoritative `llvm-mc-18` differential oracle** showing
that the in-range encodings match the system assembler byte-for-byte, while the
encoder diverges from `llvm-mc` (which rejects out-of-range shifts) exactly at
the bug boundary.

## Root Cause

In the shifted-register path (`encode_logical`):

```rust
let (shift_type, shift_amount) = if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
    let st = match kind.as_str() {
        "lsl" => 0b00u32, "lsr" => 0b01, "asr" => 0b10, "ror" => 0b11, _ => 0b00,
    };
    (st, *amount)                       // <-- amount never range-checked
} else { (0, 0) };

let word = ((sf << 31) | (opc << 29) | (0b01010 << 24) | (shift_type << 22))
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
//                   ^^^^^^^^^^^^^^^^^^^  silent mask/truncate instead of rejecting
```

There is no width check. The same `& 0x3F` masking-without-validation defect is
shared by the sibling scalar encoders `encode_orn` / `encode_eon` / `encode_bic`
/ `encode_bics` / `encode_mvn` / `encode_neg` / `encode_negs` in the same file.

## Reproduction

Confirmed by the `#[ignore]`d negative-contract witnesses in
`src/backend/arm/assembler/encoder/encode_logical_pbt.rs`. Both FAIL when run:

```
$ cargo test --lib encode_logical_pbt:: -- --ignored
test encode_logical_pbt::logical_w_reg_shift_out_of_range_masked ... FAILED
  minimal failing input: rd = 0, rn = 0, rm = 0, amount = 32, sk = 0
  → `and w0, w1, w2, lsl #32` silently accepted (UNDEFINED for W)

test encode_logical_pbt::logical_x_reg_shift_out_of_range_truncated ... FAILED
  minimal failing input: rd = 0, rn = 0, rm = 0, amount = 64, sk = 0
  → `and x0, x1, x2, lsl #64` silently truncated to `lsl #0`
```

For contrast, `llvm-mc-18` rejects the identical source:

```
$ printf 'and w0,w1,w2,lsl #32\n' | llvm-mc-18 -triple=aarch64-linux-gnu -assemble -show-encoding
<stdin>:1:1: error: expected compatible register or logical immediate
```

## Impact

- **Wrong code generation with no diagnostic.** An instruction the user wrote to
  shift by `N` (N ≥ 64 for X regs, N ≥ 32 for W regs) is assembled as a shift by
  `N mod 64`, almost never the intent. For the X case this is especially insidious:
  `lsl #64` masquerades as `lsl #0` (a no-op), so the instruction computes a
  completely different value than its source text describes.
- Violates the rest of the encoder's own contract (e.g. `encode_add_sub` rejects
  unrepresentable extended-register shifts) that out-of-range operands yield `Err`,
  not silent coercion.

## Suggested Fix

Validate the shift amount against the width before placing it, mirroring the
existing immediate-form rejection of non-bitmask values:

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "shift amount {} out of range for {}-bit logical op (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
let word = ... | ((shift_amount & 0x3F) << 10) | ...;
```

Apply the same fix to the sibling `encode_orn` / `encode_eon` / `encode_bic` /
`encode_bics` / `encode_mvn` / `encode_neg` / `encode_negs` paths.

## Regression Property

Failing properties (both `#[ignore]`d so the default suite stays green; both
genuinely FAIL under `-- --ignored`):

- `logical_w_reg_shift_out_of_range_masked`
- `logical_x_reg_shift_out_of_range_truncated`

```rust
// and w0, w1, w2, lsl #32  -> must be Err, is silently Ok(UNDEFINED)
let ops = vec![wreg(0), wreg(1), wreg(2),
               Operand::Shift { kind: "lsl".into(), amount: 32 }];
prop_assert!(encode_logical(&ops, 0).is_err());

// and x0, x1, x2, lsl #64  -> must be Err, is silently truncated to lsl #0
let ops = vec![xreg(0), xreg(1), xreg(2),
               Operand::Shift { kind: "lsl".into(), amount: 64 }];
prop_assert!(encode_logical(&ops, 0).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/54
(also https://github.com/thanhtoantnt/claudes-c-compiler/issues/55)
