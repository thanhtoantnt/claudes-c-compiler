# Bug Report: `encode_int_to_float` accepts illegal operand banks (no GP/FP validation)

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_int_to_float`
**Severity:** High

## Summary

`encode_int_to_float` (SCVTF/UCVTF) never validates register class. Requires GP integer source (`Wn`/`Xn`) to FP destination (`Sd`/`Dd`). Silently accepts GP destination and FP source, mis-deriving `ftype`/`sf` and emitting word that matches unrelated legal instruction.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;          // no FP-bank check on dest
let (rn, rn_is_64) = get_reg(operands, 1)?;   // no GP-bank check on source
let dst_name = match &operands[0] { Operand::Reg(name) => name.to_lowercase(), ... };
let ftype: u32 = if dst_name.starts_with('d') { 0b01 } else { 0b00 }; // "w" -> single
let sf: u32   = if rn_is_64 { 1 } else { 0 };                       // "d" -> sf=0
```

## Reproduction

**Input:** `scvtf w0, w0`

**Expected:** `Err` — scvtf/ucvtf: destination must be an FP register

**Actual:** `Ok(Word(0x1E220000))` — valid `SCVTF S0, W0` encoding (GP dest silently coerced)

**Minimal failing input:** n = 0

**Other failing input:** `scvtf s0, s0` (FP dest and source both GP coerced)

## Impact

Illegal instruction emitted as different legal instruction with no error. Malformed assembler input silently produces semantically wrong machine code.

## Suggested Fix

Validate operand banks:

```rust
if !is_fp_reg(&dst_name) {
    return Err("scvtf/ucvtf: destination must be an FP register".into());
}
let src_name = match &operands[1] { Operand::Reg(n) => n.to_lowercase(), _ => return Err(...) };
if is_fp_reg(&src_name) {
    return Err("scvtf/ucvtf: source must be a GP register".into());
}
```

## Regression Property

Failing property: `prop_int_to_float_rejects_wrong_operand_banks`

```rust
prop_assert!(encode_int_to_float(&[wreg(0), wreg(0)], true).is_err());   // GP dest
prop_assert!(encode_int_to_float(&[sreg(0), sreg(0)], true).is_err());   // FP source
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/130