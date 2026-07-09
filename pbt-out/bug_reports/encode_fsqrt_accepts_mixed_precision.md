# Bug — `encode_fsqrt` silently accepts mixed-precision operands

- **File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
- **Function:** `encode_fsqrt`
- **Severity:** Medium (silent mis-encoding of an invalid instruction; no `Err`)

## Minimal input

Operands: `[Operand::Reg("d0"), Operand::Reg("s0")]`  (i.e. `FSQRT D0, S0`)

## Expected vs actual

- **Expected:** `Err` — `FSQRT` is a scalar FP 1-source instruction and requires
  homogeneous precision (both operands `S`, or both `D`). A double dest with a
  single source is architecturally UNALLOCATED.
- **Actual:** `Ok(EncodeResult::Word(509722624))` = `Ok(Word(0x1E61C000))`.
  The encoder derived `ftype = 01` (double) purely from the dest prefix `d` and
  silently re-encoded the single-precision source `s0` as if it were double,
  producing `FSQRT D0, D0` rather than an error.

## Root cause

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');          // <-- ONLY the dest is inspected
let ftype = if is_double { 0b01 } else { 0b00 };
```

The source operand (`operands[1]`) is consumed only for its register *number*
via `get_reg`; its precision (`s` vs `d`) is never compared against the dest.
`is_fp_reg` / precision checks are absent.

## Impact

- A malformed `FSQRT D0, S0` is accepted and assembled into a well-formed but
  semantically different instruction (`FSQRT D0, D0`), so the user gets no
  diagnostic and the program silently computes the wrong value.
- This is the same defect already documented for the sibling functions
  `encode_fneg` and `encode_fabs`.

## Fix

Validate matching precision before encoding:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let rd_double = rd_name.starts_with('d');
let rn_double = rn_name.starts_with('d');
if rd_double != rn_double {
    return Err("fsqrt operands must have matching precision".to_string());
}
```

## Reproducing test

`prop_fsqrt_rejects_mismatched_precision_and_bank` fails on minimal input
`n = 0` at the mixed-precision assertion (`FSQRT D0, S0`).
