# BUG: `encode_bics` silently accepts architecturally-invalid shift amounts (no imm6 range validation)

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_bics`
**Severity:** Correctness (emits UNPREDICTABLE AArch64 encodings)

## Summary

`encode_bics` masks the shift amount with `& 0x3F` (6 bits) and never validates it
against the architectural limit imposed by the operand width. For a 32-bit (`W`)
register form (`sf = 0`), the `imm6` shift field is constrained to `0..=31`
(ARMv8 ARM, §C4.1.64 / §C4.1.4: *“the shift amount must be in the range 0 to
datasize-1”*). Values `32..=63` are **UNPREDICTABLE** and must be rejected at
assembly time. The current code accepts them, producing a word that decoders /
CPUs treat as undefined behaviour.

The same defect exists in the sibling logical ops that share this shape
(`encode_bic`, `encode_orn`, `encode_eon`, `encode_mvn`, `encode_and`/`orr`/`eor`
shifted-register forms, and `encode_neg`/`encode_negs` via SUB/SUBS), all of which
apply `(shift_amount & 0x3F)` with no `sf`-aware bound check.

## Reproduction

```text
bics w0, w0, w0, lsl #32   →  Ok(Word(0x6A208000))
```

- `sf = 0` (32-bit), `opc = 11`, shift-type `00` (lsl), `N = 1`, `imm6 = 32`.
- `0x6A208000` is an UNPREDICTABLE encoding (`sf=0` with `imm6 ≥ 32`).

PBT property `bics_w_register_rejects_shift_above_31` shrinks to exactly this case:
`rd = 0, rn = 0, rm = 0, amount = 32, sk = 0`.

## Root cause

```rust
let word = (sf << 31) | (0b11 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

The `& 0x3F` only keeps the low 6 bits; there is no check that
`shift_amount <= if is_64 { 63 } else { 31 }` before encoding, and no `Err`
return for out-of-range values.

## Suggested fix

Validate the shift amount against the register width before encoding (applies to
all logical/arithmetic shifted-register encoders):

```rust
let max_shift = if is_64 { 63 } else { 31 };
if shift_amount > max_shift {
    return Err(format!(
        "bics: shift amount {} out of range for {}-bit register (max {})",
        shift_amount, if is_64 { 64 } else { 32 }, max_shift
    ));
}
```

## PBT coverage

| Property | Status |
|---|---|
| `bics_field_placement` | PASS |
| `bics_sf_tracks_width` | PASS |
| `bics_differs_from_bic_only_in_opc` | PASS |
| `bics_rejects_too_few_operands` | PASS |
| `bics_w_register_rejects_shift_above_31` | **FAIL** (this bug) |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/3
