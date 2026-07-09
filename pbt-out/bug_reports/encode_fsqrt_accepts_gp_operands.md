# Bug — `encode_fsqrt` silently accepts GP-bank (non-FP) operands

- **File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
- **Function:** `encode_fsqrt`
- **Severity:** Medium (silent mis-encoding of an invalid instruction; no `Err`)

## Minimal input

Operands: `[Operand::Reg("x0"), Operand::Reg("x1")]`  (i.e. `FSQRT X0, X1`)

## Expected vs actual

- **Expected:** `Err` — `FSQRT` operates on scalar FP registers (`S`/`D`) only.
  General-purpose (`X`/`W`) operands are not valid; the encoding is
  architecturally UNALLOCATED for this instruction class.
- **Actual:** `Ok(EncodeResult::Word(0x1E21C000))` with `ftype = 00` and
  `sf = 0` — i.e. it emits `FSQRT S0, S1` using the GP register *numbers* (0
  and 1), treating GP registers as if they were single-precision FP registers.

## Root cause

```rust
let is_double = rd_name.starts_with('d');          // <-- GP 'x'/'w' => false => ftype 00
let ftype = if is_double { 0b01 } else { 0b00 };
```

`get_reg` (via `parse_reg_num`) happily parses `x`/`w` registers, so the GP
register numbers flow straight into the `Rn`/`Rd` fields. The bank of the
operands is never checked; `is_fp_reg` (which exists in `mod.rs`) is never
called.

## Impact

- A nonsensical `FSQRT X0, X1` is assembled without diagnostic into what
  decodes as `FSQRT S0, S1`, hiding a user error and producing wrong runtime
  behavior.
- Same defect class as `encode_fneg` / `encode_fabs`.

## Fix

Require FP-register operands before encoding:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
if !is_fp_reg(&rd_name) || !is_fp_reg(&rn_name) {
    return Err("fsqrt requires FP-register operands".to_string());
}
```

## Reproducing test

`prop_fsqrt_rejects_mismatched_precision_and_bank` exercises the GP-bank case
(`FSQRT X0, X0`); combined with the mixed-precision case it fails on minimal
input `n = 0`. (Split out: the GP assertion alone fails for any `n` in `0..32`.)
