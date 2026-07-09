# Bug: `encode_int_to_float` accepts illegal operand banks (no GP/FP validation)

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_int_to_float(operands: &[Operand], is_signed: bool) -> Result<EncodeResult, String>`
**Discovered by:** property-based testing (`prop_int_to_float_rejects_wrong_operand_banks`, FAILS)

## Summary

`encode_int_to_float` (the SCVTF/UCVTF encoder) never validates the *register
class* of either operand. SCVTF/UCVTF convert a **GP integer source**
(`Wn`/`Xn`) to an **FP destination** (`Sd`/`Dd`); any other bank combination is
illegal. Instead the encoder silently accepts a GP destination and an FP
source, mis-deriving `ftype`/`sf` from the operand name prefix and emitting a
word that bit-for-bit matches an unrelated legal instruction.

## Minimal failing case

```rust
encode_int_to_float(&[Operand::Reg("w0".into()), Operand::Reg("w0".into())], true)
  == Ok(EncodeResult::Word(505544704))   // == 0x1E220000
```

`0x1E220000` is the valid encoding of **`SCVTF S0, W0`**. So the illegal
`SCVTF W0, W0` silently round-trips into the legal `SCVTF S0, W0` — a word with
completely wrong register-class semantics. Minimal input: `n = 0`.

## Property output

```text
prop_int_to_float_rejects_wrong_operand_banks ... FAILED
Test failed: GP destination (w0) must be rejected; SCVTF/UCVTF dest must be FP,
             got Ok(Word(505544704))
minimal failing input: n = 0
```

## Root cause

```rust
let (rd, _) = get_reg(operands, 0)?;          // no FP-bank check on dest
let (rn, rn_is_64) = get_reg(operands, 1)?;   // no GP-bank check on source
let dst_name = match &operands[0] { Operand::Reg(name) => name.to_lowercase(), ... };
let ftype: u32 = if dst_name.starts_with('d') { 0b01 } else { 0b00 }; // "w" -> single
let sf: u32   = if rn_is_64 { 1 } else { 0 };                       // "d" -> sf=0
```

A GP dest name (e.g. `"w0"`) does not start with `'d'`, so `ftype` defaults to
`00` (single precision). An FP source (`"d0"`) is not a 64-bit GP register, so
`sf` defaults to `0`. Neither illegal operand is ever rejected.

## Suggested fix

Validate operand banks before computing fields:

```rust
if !is_fp_reg(&dst_name) { return Err("scvtf/ucvtf: destination must be an FP register".into()); }
let src_name = match &operands[1] { Operand::Reg(n) => n.to_lowercase(), _ => return Err(...) };
if is_fp_reg(&src_name)  { return Err("scvtf/ucvtf: source must be a GP register".into()); }
```

After the fix, `prop_int_to_float_rejects_wrong_operand_banks` should pass.

## Severity

High — an illegal instruction is emitted as a *different* legal instruction
with no error, so malformed assembler input silently produces semantically
wrong machine code.
