# Bug: `encode_mvn` silently accepts UNDEFINED W-register shift amounts

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_mvn` (scalar/shifted-register path)

## Summary
`encode_mvn` masks the shift amount with `& 0x3F` and never validates the
register-width-dependent legal range. For a 32-bit (`W`) destination, shift
amounts of 32–63 are **UNDEFINED** per the ARMv8 ARM (C4.1.4: for `sf=0` the
imm6 shift amount must be `0..=31`), yet the encoder emits a bogus 32-bit word
instead of returning `Err`. GAS and LLVM-MC both reject these.

## Root cause
```rust
let word = (sf << 31) | (0b01 << 29) | (0b01010 << 24) | (shift_type << 22) | (1 << 21)
    | (rm << 16) | ((shift_amount & 0x3F) << 10) | (0b11111 << 5) | rd;
```
`shift_amount` is taken verbatim from the parsed `Operand::Shift { amount }` and
only masked into 6 bits — there is no check that, when `sf == 0` (`Wd`),
`amount <= 31`.

## Reproduction
Property `mvn_w_reg_rejects_shift_above_31` (added in this file's `mod tests`)
fails on the minimal input:

```
mvn w0, w0, lsl #32          -> encode_mvn returns Ok(0x2A2003E0)  [UNDEFINED]
mvn w0, w0, lsr #63 / asr #48 / ror #40  -> likewise silently encoded
```

Expected: `Err`. Actual: `Ok(EncodeResult::Word(...))` with `imm6 = amount`.

## Impact
Emits instructions whose execution is architecturally UNPREDICTABLE on every
AArch64 implementation. This is a correctness defect for the backend: programs
using `mvn wd, wm, <shift> #N` with N>=32 silently mis-assemble rather than
fail at assembly time.

## Notes
- The 4 positive properties I added all PASS (field placement, sf width
  tracking, shift-kind -> 2-bit mapping, and the `mvn == orn rd, xzr, rm`
  algebraic-alias differential oracle). The encoding is otherwise correct for
  in-range inputs.
- The same `& 0x3F`-without-validation pattern (and the same class of bug)
  appears in the sibling encoders `encode_orn`, `encode_eon`, `encode_bics`,
  `encode_bic`, `encode_logical`, `encode_neg`, `encode_negs`, and the
  shifted-register path of `encode_add_sub`. The pre-existing negative-contract
  tests for those functions fail in the same way, confirming this is a
  systematic gap, not a one-off.

## Suggested fix
After resolving the shift, reject out-of-range amounts per width:
```rust
let max_shift = if is_64 { 63 } else { 31 };   // ROR additionally requires >= 1
if shift_amount > max_shift {
    return Err(format!("mvn: shift amount {} out of range for {}-bit register",
                       shift_amount, if is_64 { 64 } else { 32 }));
}
```
