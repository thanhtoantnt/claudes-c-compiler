# Bug: `encode_add_sub` shifted-register form silently accepts mixed register widths

**Function:** `encode_add_sub` (shifted-register branch) — `src/backend/arm/assembler/encoder/data_processing.rs`
**Detected by:** Property-based test — negative/error contract (differential vs `clang --target=aarch64`)
**Severity:** Medium (assembles a valid-but-semantically-wrong instruction with no diagnostic)

## Law

The ARMv8 ARM *Add/subtract (shifted register)* form requires `<Rd>, <Rn>, <Rm>` to all
share one register width — all `W` (32-bit) or all `X` (64-bit). The `sf` bit (bit 31)
then selects the width for the whole instruction. A mixed-width spelling has no valid
encoding and must be rejected. `clang --target=aarch64-linux-gnu` rejects it:

```text
$ echo '.text
add x0, w1, x2' | clang --target=aarch64-linux-gnu -c -x assembler -
error: invalid operand for instruction

$ echo '.text
add w0, x1, x2' | clang --target=aarch64-linux-gnu -c -x assembler -
error: invalid operand for instruction
```

## Root cause

The shifted-register branch reads the destination width via `get_reg(operands, 0)` (which
returns `is_64`) and derives `sf` from it, but reads `Rm` via `parse_reg_num(rm_name)`,
which returns only the 5-bit register **number** and discards its width. `Rn` is likewise
read with `get_reg` but its `is_64` is bound to `_`. So no width-coherence check is
possible:

```rust
let (rd, is_64) = get_reg(operands, 0)?;   // width kept -> drives sf
let (rn, _) = get_reg(operands, 1)?;       // is_64 DISCARDED
...
let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;   // width DISCARDED (number only)
...
let word = ((sf << 31) | (op << 30) | (s_bit << 29) | (0b01011 << 24) | (shift_type << 22))
         | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

A `W`-numbered and an `X`-numbered register with the same index land in the same 5-bit
field, so `add w0, w0, x0` is emitted as a 32-bit instruction with `Rm = 0`, indistinguishable
at the encoding level from a correctly typed `add w0, w0, w0`.

## Minimal input / reproducer

```
witness property:  addsub_shifted_reg_rejects_mixed_widths
minimal failing input: n = 0, rd_is_x = false, rn_is_x = false, rm_is_x = true
```

Source: `add w0, w0, x0`

- **Expected:** `Err` — operands do not share one register width.
- **Actual:** `Ok(Word(0x0B000000))` — a 32-bit `add w0, w0, w0` (`sf = 0`, `Rm = 0`)
  with no diagnostic. Every mixed-width combination (e.g. `add x0, w1, w2`,
  `add w0, x1, x2`, `add x0, x1, w2`) is likewise silently mis-typed.

## Impact

Silent mis-compilation: the assembler accepts a malformed source line and emits an
instruction whose `sf` bit does not match the (textual) source operand widths. The
corruption is invisible at the encoding level and surfaces only as silent mis-execution
on target or as a divergence from a reference assembler.

## Suggested fix

Validate width coherence across all three operands:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
...
let rm = parse_reg_num(rm_name).ok_or("invalid rm")?;
let rm_64 = is_64bit_reg(rm_name);            // need the width, not just the number
if rn_64 != is_64 || rm_64 != is_64 {
    return Err(format!(
        "add/sub operands must all share the destination's register width ({}-bit)",
        if is_64 { 64 } else { 32 }));
}
```

## Regression property

Failing witness (marked `#[ignore]` so the default suite stays green):

```rust
// in src/backend/arm/assembler/encoder/data_processing_addsub_div_bitmask_pbt.rs
#[ignore = "documented bug: add/sub shifted-register accepts mixed register widths (clang rejects)"]
#[test]
fn addsub_shifted_reg_rejects_mixed_widths(n in 0u32..=30, rd_is_x in bool, rn_is_x in bool, rm_is_x in bool) {
    prop_assume!(!(rd_is_x == rn_is_x && rn_is_x == rm_is_x));
    let rd = if rd_is_x { xreg(n) } else { wreg(n) };
    let rn = if rn_is_x { xreg(n) } else { wreg(n) };
    let rm = if rm_is_x { xreg(n) } else { wreg(n) };
    prop_assert!(encode_add_sub(&[rd, rn, rm], false, false).is_err());
}
```

Run the witness:

```text
cargo test --lib data_processing_addsub_div_bitmask_pbt::addsub_shifted_reg_rejects_mixed_widths -- --ignored
```

## Related reports (sibling functions, same defect class)

- [`encode_div_silent_width_acceptance.md`](encode_div_silent_width_acceptance.md) — SDIV/UDIV
- [`encode_adc_silent_mixed_width.md`](encode_adc_silent_mixed_width.md) — ADC/SBC
- [`encode_mul_mixed_width_no_validation.md`](encode_mul_mixed_width_no_validation.md) — MUL
- [`encode_msub_mixed_width_operands.md`](encode_msub_mixed_width_operands.md) — MSUB

This report is filed for the **affected function itself** (`encode_add_sub`), per
one-report-per-affected-function.
