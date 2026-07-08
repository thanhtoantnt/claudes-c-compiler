# Bug Report — `encode_tst` accepts reserved / silently-truncated shift amounts

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` — `encode_tst`
(root cause in `encode_logical`, `data_processing.rs`)

**Severity:** Medium (incorrect AArch64 machine code emitted; MIS-assembly)

## Summary

`encode_tst` forwards `TST Rn, Rm, shift #amount` to `encode_logical`, which
computes the `imm6` shift-amount field as `shift_amount & 0x3F`. This **silently
accepts / truncates** shift amounts that are architecturally out of range instead
of rejecting them with `Err`:

* **32-bit register (sf = 0):** legal shift is `0..=31`; `imm6 > 0x1f` is
  **RESERVED** (ARM ARM, "Logical (shifted register)", C4.1.4 / C6.2.x).
* **64-bit register (sf = 1):** legal shift is `0..=63`; amounts `>= 64` are
  masked down to a wrong value by `& 0x3F` (e.g. `lsl #64` → `lsl #0`,
  `lsl #100` → `lsl #36`).

GAS and `llvm-mc` reject these inputs. No cited spec permits wrapping/truncation
of the shift-amount field.

## Reproduction (failing property-based test)

`prop_encode_tst_tests::prop_rejects_oversized_shift`, minimal failing input:

```
tst w5, w6, lsl #32      # is_64 = false, shift amount = 32 (max legal = 31)
```

```
$ cargo test --lib prop_encode_tst
test ...::prop_rejects_oversized_shift ... FAILED
minimal failing input: is_64 = false, excess = 1
  → Ok(Word(0x6A0680BF))
```

Decoded result `0x6A0680BF`:

| field    | bits    | value | note                          |
|----------|---------|-------|-------------------------------|
| sf       | [31]    | 0     | 32-bit                        |
| opc      | [30:29] | 0b11  | ANDS ✓                        |
| opcode   | [28:24] | 01010 | logical shifted reg ✓         |
| Rm       | [20:16] | 6     | ✓                             |
| **imm6** | **[15:10]** | **32** | **RESERVED (must be ≤ 31)** |
| Rn       | [9:5]   | 5     | ✓                             |
| Rd       | [4:0]   | 31    | zr ✓                          |

The encoder emitted a well-formed-looking word whose `imm6` field is reserved —
behaviour is CONSTRAINED UNPREDICTABLE on real hardware.

## Suggested fix

In `encode_logical` (shifted-register branch, `data_processing.rs`), validate
the shift amount before masking:

```rust
let limit = if is_64 { 63 } else { 31 };
if shift_amount > limit {
    return Err(format!("shift amount {} out of range for {}-bit register",
                       shift_amount, if is_64 { 64 } else { 32 }));
}
// ... then use shift_amount directly (the & 0x3F mask becomes a no-op).
```

This also fixes the same latent bug for `encode_and` / `encode_orr` / `encode_eor`
/ `encode_bic` / etc., which share `encode_logical`.

## Properties added

Appended module `prop_encode_tst_tests` to `compare_branch.rs` (6 properties,
`proptest!`):

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| A | `prop_register_form_structure` | structural / field-placement | ✅ pass |
| B | `prop_immediate_form_structure` | structural / field-placement (incl. N=RES0 for sf=0) | ✅ pass |
| C | `prop_sf_bit_is_bit31` | differential (32- vs 64-bit) | ✅ pass |
| D | `prop_tst_is_ands_and_differs_only_in_opc` | differential vs AND | ✅ pass |
| E | `prop_rejects_invalid_inputs` | negative contract (#0 imm, x32, arity) | ✅ pass |
| F | `prop_rejects_oversized_shift` | negative contract (shift range) | ❌ **FAIL → this bug** |
