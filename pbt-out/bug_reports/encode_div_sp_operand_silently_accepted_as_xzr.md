# Bug Report: `encode_div` silently accepts SP operands as XZR

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_div`

## Summary

`encode_div` parses operands through the shared generic register path, which maps `sp`/`wsp` to register number 31. DIV instructions do not have an SP form; register 31 in this encoding is XZR/WZR. The encoder therefore accepts invalid SP operands and silently encodes them as zero-register operands.

## Reproduction

Characterization from the PBT campaign:

```text
sdiv x0, x1, sp
```

Actual behavior: returns `Ok(Word(_))` with `Rm = 31`, equivalent to `sdiv x0, x1, xzr`.

## Impact

Invalid division source is accepted and assembled as a different instruction than written. In the divisor position this changes behavior drastically, since XZR is zero.

## Suggested fix

Use a register parser for this instruction class that rejects SP/WSP:

```rust
let rm_name = get_reg_name(operands, 2)?;
if rm_name.eq_ignore_ascii_case("sp") || rm_name.eq_ignore_ascii_case("wsp") {
    return Err("div operands cannot use SP/WSP".to_string());
}
```

## Regression property

Failing property: `div_rejects_sp_operands`

```rust
prop_assert!(encode_sdiv(&[xreg(0), xreg(1), Operand::Reg("sp".into())]).is_err());
```
