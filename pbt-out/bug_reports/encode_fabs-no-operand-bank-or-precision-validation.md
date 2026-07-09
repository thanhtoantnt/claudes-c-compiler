# Bug: `encode_fabs` does not validate operand bank or precision homogeneity

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fabs`
**Severity:** Medium (silent mis-encoding → incorrect machine code, no error)
**Found by:** `prop_fabs_rejects_mismatched_precision_and_bank` (property-based test, FAILS by design)

## Summary

`encode_fabs` derives the `ftype` precision field **only** from `operands[0]`
(the destination) and never validates:
1. that both operands are FP-register operands (not GP `x`/`w`), and
2. that the source operand's precision **matches** the destination's.

Per ARMv8-A, `FABS` is a "Floating-point data-processing (1 source)" instruction
that requires **homogeneous** FP-register operands (`FABS Sd,Sn` or `FABS Dd,Dn`).
Mixed-precision operands and GP-bank operands are illegal and must be rejected.

## Reproduction

```
encode_fabs([Reg("d0"), Reg("s0")])
  => Ok(Word(0x1E60C000))     // WRONG: silently accepted, treated as FABS D0,D0

encode_fabs([Reg("x0"), Reg("x0")])
  => Ok(Word(0x1E20C000))     // WRONG: GP operands silently encoded as FABS S0,S0
```

Minimal failing input from proptest: `n = 0`.
- `Ok(Word(509657088))` = `0x1E60C000` = the **double-precision** `FABS D0,D0`
  encoding, even though the source was `S0` (single). The single-precision
  source is silently coerced into a double-precision register field.

## Root cause

```rust
pub(crate) fn encode_fabs(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
    let is_double = rd_name.starts_with('d');          // <-- dest-only
    let ftype = if is_double { 0b01 } else { 0b00 };   // <-- dest-only
    // FABS: 0 00 11110 ftype 1 0000 01 10000 Rn Rd
    let word = (0b00011110 << 24) | (ftype << 22) | (0b100000 << 16)
             | (0b110000 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))                       // <-- no validation
}
```

There is no check that `operands[1]` is an FP register of the same width as
`operands[0]`, and no check that either operand belongs to the FP register bank
(`s`/`d`/`h`) rather than the GP bank (`w`/`x`).

## Impact

- An assembler user writing `FABS D0, S0` (a typo / illegal instruction) gets a
  **successful** `Word(0x1E60C000)` back — i.e. `FABS D0, D0` — instead of an
  error. The single→double mismatch is silently lost, producing semantically
  wrong machine code.
- GP operands (`FABS X0, X0`) are likewise silently encoded instead of rejected.

## Note on encoding correctness (verified, not a bug)

The bit layout itself is **correct**. For homogeneous-precision operands the
function emits the canonical ARMv8 encoding:
- `FABS S0,S0` = `0x1E20C000`, `FABS D0,D0` = `0x1E60C000`
- opcode `[20:15]` = `000001`, fixed `[14:10]` = `10000`, bit 21 = 1,
  `[31:24]` = `0x1E`, sf always 0, ftype from dest.

Confirmed passing:
`prop_fabs_places_fields`, `prop_fabs_ftype_from_dest`,
`prop_fabs_is_deterministic`, `prop_fabs_rejects_out_of_range_reg`.

This bug is the same class of defect already documented for the sibling
1-source/2-source FP encoders in this file (`encode_fneg`, `encode_fp_1src`,
`encode_fp_arith`, `encode_fcmp`, `encode_fmadd_fmsub`).

## Suggested fix

After parsing both operands, verify homogeneous FP precision and reject
non-FP banks before encoding, e.g.:

```rust
let rn_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => return Err("...".into()) };
if rd_name.starts_with('d') != rn_name.starts_with('d') {
    return Err("FABS operands must have matching precision".into());
}
// optionally: reject GP banks (w/x) explicitly
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/170
