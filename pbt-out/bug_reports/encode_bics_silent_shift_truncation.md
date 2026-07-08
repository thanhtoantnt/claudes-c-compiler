# Bug Report: `encode_bics` silently truncates out-of-range shift amounts

**Function:** `encode_bics` in `src/backend/arm/assembler/encoder/data_processing.rs`
**Severity:** Low–Medium (produces an unintended, semantically-different instruction with no diagnostic)
**Discovered by:** property-based test `bics_props::bics_rejects_oversized_shift` (FAILING)

## Summary

`encode_bics` (and identically `encode_bic`, `encode_orn`, `encode_eon`) accept a shift
amount operand and fold it into the `imm6` field (bits 15:10) of the AArch64 *Logical
(shifted register)* encoding using a bare mask `& 0x3F`:

```rust
let word = (sf << 31) | (0b11 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (rn << 5) | rd;
```

Because `imm6` is only 6 bits wide, any requested shift amount `> 63` is **silently
truncated** rather than rejected. The assembler emits a different instruction than the
source text requested, with no error.

## Reproduction

Minimal input found by the property test:
```
bics x0, x0, x0, lsl #64      // requested shift amount = 64
```
`encode_bics` returns `Ok(...)` and emits the same word as `lsl #0` (since `64 & 0x3F == 0`).
The same happens for any `amount >= 64`.

## Spec basis

ARMv8 ARM, §C4.1.4 *Logical (shifted register)*: the `imm6` field is 6 bits. There is no
defined encoding for a shift amount outside `[0, 63]` for 64-bit operands (and `[0, 31]`
for 32-bit operands, see related note below). A conforming assembler must reject these.

## Expected vs. actual

| Input                       | Expected            | Actual                          |
|-----------------------------|---------------------|---------------------------------|
| `bics x0,x0,x0, lsl #64`    | `Err(...)`          | `Ok` → encoded as `lsl #0`      |
| `bics x0,x0,x0, lsl #4095`  | `Err(...)`          | `Ok` → encoded as `lsl #63`     |

## Suggested fix

Validate the shift amount against the register width before encoding:

```rust
let max_shift = if is_64 { 63 } else { 31 };   // 32-bit operands: imm6 bit5 must be 0
if shift_amount > max_shift {
    return Err(format!("shift amount {} out of range for {}-register bics", shift_amount, if is_64 { 64 } else { 32 }));
}
```

## Related / broader impact

The identical `& 0x3F` masking pattern appears in `encode_bic`, `encode_orn`,
`encode_eon`, `encode_mvn`, `encode_neg`, `encode_negs`, and the logical/shifted
`encode_add_sub` path — all share this defect. A second, narrower issue applies to
**32-bit (W) register** operands: the ARMv8 spec requires `imm6` bit 5 to be 0 for W-form
logical/shifted-register instructions, i.e. shift amounts 32–63 are UNPREDICTABLE, but the
encoder accepts them silently. Applying the width-aware bound above fixes both.

## Verified-correct behavior

The accompanying passing properties confirm everything else about `encode_bics` is correct:
field placement (`opc=11`, `01010`, `N=1`), all four shift-kind encodings, `sf` width
tracking, and — via the cross-instruction differential `bics_word ^ bic_word == 0x6000_0000`
— that BICS differs from BIC *only* in the opc field, exactly as the flag-setting variant
should.
