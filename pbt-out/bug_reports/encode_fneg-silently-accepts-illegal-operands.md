# Bug Report — `encode_fneg` silently accepts illegal operands

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `pub(crate) fn encode_fneg(operands: &[Operand]) -> Result<EncodeResult, String>`

## Summary
`encode_fneg` performs **no operand-bank or source-precision validation**. It derives
`ftype` solely from the **destination** register's prefix (`'d'` ⇒ double, else single)
and trusts `get_reg`/`parse_reg_num` for everything else. As a result it silently encodes
instructions that are **UNPREDICTABLE / unallocated** in the ARMv8-A ISA:

* **Mixed precision** — e.g. `FNEG D0, S0` (double dest, single source) is accepted and
  encoded as a *double-precision* FNEG.
* **Wrong bank (GP operands)** — e.g. `FNEG X0, X0` is accepted and encoded as a
  single-precision FNEG (ftype mis-derived as `00` from the non-`d` prefix).

This is the same class of defect already documented for the sibling functions
`encode_fp_arith`, `encode_fp_1src`, `encode_fcmp`, and `encode_int_to_float` (see
`BUGS_fp_arith.md` and the inline `FINDING — FAILS` properties in this file).

## Reproduction (property-based, minimal failing input)
Property `prop_fneg_rejects_mismatched_precision_and_bank` with `n = 0`:

```text
FNEG D0, S0  -> Ok(Word(0x1E614000))   // double FNEG encoding; source precision ignored
FNEG X0, X0  -> Ok(Word(0x1E214000))   // single FNEG encoding; GP operands accepted
```

`0x1E614000` is the *correct* encoding for `FNEG D0, D0`, not for `FNEG D0, S0`.
`0x1E214000` is the correct encoding for `FNEG S0, S0`, not for `FNEG X0, X0`.

## Root cause
```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');          // only the dest is inspected
let ftype = if is_double { 0b01 } else { 0b00 };
```
* `operands[1]` (the source) is never checked for FP-ness or for matching precision.
* Neither operand is checked for being an FP-bank register (`s`/`d`/`h`/`q`).
* `get_reg` happily accepts any of `x|w|d|s|q|v|h|b`, so GP-bank operands slip through.

## Impact
* **Correctness:** emitting unallocated/UNPREDICTABLE encodings into the instruction
  stream. A double-precision FNEG with a single-precision source register field is an
  illegal combination; behavior on real hardware (and assemblers like `as`/`llvm-mc`)
  is to reject it.
* **Silent failure:** returns `Ok`, so callers have no signal that the mnemonic was
  malformed — the bad word is emitted into the output.

## Suggested fix
After resolving the registers, validate that both operands are FP-bank registers and that
their precision matches the destination-derived `ftype`:

```rust
let rd_is_fp = is_fp_reg(&rd_name);
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let rn_is_fp = is_fp_reg(&rn_name);
if !rd_is_fp || !rn_is_fp {
    return Err(format!("fneg requires FP-register operands, got {}, {}", rd_name, rn_name));
}
let rn_double = rn_name.starts_with('d');
if rn_double != is_double {
    return Err("fneg source and destination precision must match".to_string());
}
```
(`is_fp_reg` already exists in the encoder module and is used by `encode_fmov`.)

## Property results
| Property | Result |
|---|---|
| `prop_fneg_places_fields` (reference / field layout, opcode == 000010) | ✅ PASS |
| `prop_fneg_ftype_from_dest` (precision derivation) | ✅ PASS |
| `prop_fneg_is_deterministic` | ✅ PASS |
| `prop_fneg_rejects_out_of_range_reg` (n ≥ 32 rejected) | ✅ PASS |
| `prop_fneg_rejects_mismatched_precision_and_bank` | ❌ **FAIL** (this finding) |

The four passing properties confirm the **encoding arithmetic itself is correct**
(opcode `000010`, ftype, fixed bits `0x1E214000`/`0x1E614000`, register field placement);
only operand validation is missing.
