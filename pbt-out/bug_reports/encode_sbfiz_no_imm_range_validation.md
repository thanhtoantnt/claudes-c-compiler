# Bug Report — `encode_sbfiz` silently accepts out-of-range `lsb`/`width` and panics on `width == 0`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_sbfiz`
**Severity:** Medium (silent miscompilation for oversized/`width==0` inputs in release builds; **debug-mode panic** for `width == 0`, crashing the host assembler)

## Summary

`encode_sbfiz` reads its two immediate operands with `get_imm(...)? as u32` and
folds them into the 32-bit encoding word with **no range validation**:

```rust
let lsb = get_imm(operands, 2)? as u32;
let width = get_imm(operands, 3)? as u32;
let regsize = if is_64 { 64u32 } else { 32 };
...
let immr = (regsize.wrapping_sub(lsb)) & (regsize - 1);   // line 69
let imms = width - 1;                                       // line 70  ← underflow panic
let word = (sf << 31) | (0b100110 << 23) | (n << 22)
         | (immr << 16) | (imms << 10) | (rn << 5) | rd;
```

`SBFIZ Rd, Rn, #lsb, #width` is the alias of
`SBFM Rd, Rn, #(-lsb MOD regsize), #(width-1)`. Per the ARM Architecture
Reference Manual (Bitfield / SBFIZ), the operands must satisfy:

* 64-bit: `0 <= lsb <= 63`, `1 <= width <= 64 - lsb`
* 32-bit: `0 <= lsb <= 31`, `1 <= width <= 32 - lsb`

so that `immr` and `imms` each fit their 6-bit fields (`[21:16]` / `[15:10]`).
An assembler **must reject** any value outside these ranges. The current code
performs no check, so:

* `width == 0` computes `imms = width - 1` → **`attempt to subtract with
  overflow` panic in debug mode** (aborting the whole assembler host process),
  and a silent wrap to `imms = u32::MAX` in release mode (corrupting the whole
  opcode word).
* `width >= 65` overflows `imms = width - 1` out of its 6-bit field into the
  `Rn` region (`[9:5]`) and beyond, producing a semantically wrong instruction.
* `lsb > 63` does not error: `immr = (regsize.wrapping_sub(lsb)) & (regsize-1)`
  silently maps the out-of-range `lsb` to an arbitrary in-range garbage `immr`.
* Negative immediates (e.g. `-3`) are accepted via the `i64 as u32` cast and
  become enormous positive values that corrupt multiple fields.

## Reproduction (property-based test)

`prop_encode_sbfiz_tests::prop_rejects_out_of_range_immediates` fails on the
**first** generated input:

```
minimal failing input: is_64 = false, over_lsb = 64, over_width = 65, neg = -3
panicked at bitfield.rs:2187:
  width=0 (imms underflow) should be Err but PANICKED
```

The panic originates inside `encode_sbfiz` itself at `bitfield.rs:70`
(`let imms = width - 1;`), which the property's `catch_unwind` observes and
reports as a contract violation: `width == 0` should return `Err`, but instead
the encoder **panics the thread** (debug) or silently wraps (release). The other
three cases (`over_lsb`, `over_width`, negative) return `Ok` with a word whose
fields no longer correspond to the inputs — a silent miscompile.

## Expected behavior

`encode_sbfiz` should return `Err(...)` whenever `lsb`/`width` are outside the
`SBFIZ` operand ranges above (and crucially should **reject `width == 0`**
rather than underflowing), matching the contract every validated-field encoder
is expected to uphold.

## Suggested fix

```rust
let lsb_i = get_imm(operands, 2)?;
let width_i = get_imm(operands, 3)?;
if lsb_i < 0 || width_i < 1 {
    return Err(format!("SBFIZ needs 0<=lsb and width>=1, got lsb={} width={}", lsb_i, width_i));
}
let regsize: i64 = if is_64 { 64 } else { 32 };
if lsb_i >= regsize || lsb_i + width_i > regsize {
    return Err(format!("SBFIZ lsb+width exceeds regsize ({}), got lsb={} width={}", regsize, lsb_i, width_i));
}
let lsb = lsb_i as u32;
let width = width_i as u32;
```

(Bound `lsb`/`width` on the `i64` values *before* the `as u32` cast so negatives
and `width == 0` are rejected and can never reach the `width - 1` underflow.)

## Scope

The same `as u32`-no-validation pattern (and the same bug class) is present in
the sibling encoders in this file: `encode_sbfm`, `encode_ubfm`, `encode_bfm`
(already documented in `pbt-out/bug_reports/`), and `encode_ubfiz`, `encode_bfi`
which share the identical `imms = width - 1` underflow panic. A shared
validation helper for bitfield `lsb`/`width` operands would address all of them.

## Test status (this campaign)

| Property | Oracle | Result |
|---|---|---|
| `prop_sbfiz_field_placement` | structural (opc=00, N==sf, immr/imms reconstruct) | PASS |
| `prop_immr_is_neg_lsb_mod_regsize` | alias formula | PASS |
| `prop_sbfiz_equals_sbfm_with_converted_immediates` | differential vs `encode_sbfm` | PASS |
| `prop_sbfiz_xor_ubfiz_is_only_bit_30` | differential vs `encode_ubfiz` | PASS |
| `prop_rejects_out_of_range_immediates` | negative contract | **FAIL — this bug** |
| `prop_rejects_malformed_operands` | negative contract | PASS |

Run: `cargo test --lib prop_encode_sbfiz` → `5 passed; 1 failed`.
