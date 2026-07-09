# B3 — `encode_fcvt_rounding`: operand banks not validated

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fcvt_rounding(operands, rmode, opcode)`
**Witness test:** `prop_fcvt_rounding_rejects_wrong_bank_operands`
(`src/backend/arm/assembler/encoder/fp_scalar_fcvt_rounding_pbt.rs`, `#[ignore]`)
**Reproduce:** `cargo test prop_fcvt_rounding_rejects_wrong_bank_operands -- --ignored`

## Minimal inputs

```rust
// (a) FP destination — must be a GP register (W/X).
encode_fcvt_rounding(&[Operand::Reg("d0".into()), Operand::Reg("s0".into())],
                     /* rmode */ 0b11, /* opcode */ 0)

// (b) GP source — must be an FP register (S/D).
encode_fcvt_rounding(&[Operand::Reg("w0".into()), Operand::Reg("x0".into())],
                     /* rmode */ 0b11, /* opcode */ 0)
```

## Expected vs actual

- **Expected:** `Err` — float-to-int FCVT\* requires a GP destination (W/X) and
  an FP source (S/D).
- **Actual:** `Ok(...)` for both. In (a) the FP dest `d0` is treated as a 32-bit
  GP dest (`sf = 0`); in (b) the GP source `x0` is treated as single-precision
  (`ftype = 00`).

## Root cause

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;
let src_name = match &operands[1] {
    Operand::Reg(name) => name.to_lowercase(),
    _ => return Err("fcvt*: expected register source".to_string()),
};
let ftype: u32 = if src_name.starts_with('d') { 0b01 } else { 0b00 };
let sf: u32 = if rd_is_64 { 1 } else { 0 };
```

`sf` is derived from the dest width and `ftype` from the source prefix, but
**neither operand's bank is validated**: an FP destination (S/D) and a GP source
(W/X) are accepted without error.

## Impact

The encoder silently assembles operands the architecture treats as
UNDEFINED/unallocatable for the float-to-int conversion class, producing a word
that is not a valid FCVT\* instruction while returning `Ok`.

## Fix

Validate banks before packing:

```rust
// destination must be a GP register (W/X)
if is_fp_reg(&match &operands[0] { Operand::Reg(n) => n.clone(), _ => String::new() }) {
    return Err("fcvt*: destination must be a GP register (W/X)".to_string());
}
// source must be an FP register (S/D)
if !is_fp_reg(&match &operands[1] { Operand::Reg(n) => n.clone(), _ => String::new() }) {
    return Err("fcvt*: source must be an FP register (S/D)".to_string());
}
```
