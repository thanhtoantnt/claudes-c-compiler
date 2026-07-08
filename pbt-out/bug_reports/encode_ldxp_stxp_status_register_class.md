# Bug Report: `encode_ldxp_stxp` accepts X register as STXP status register

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldxp_stxp`

## Summary

For STXP/STLXP, the status/result register `Rs` must be a W register. `encode_ldxp_stxp` parses it with the generic register parser and accepts X registers, silently encoding only the register number.

## Reproduction

Direct probe from the PBT campaign:

```text
stxp x9, x0, x1, [x2]
```

Actual result:

```text
Ok(Word(_))
```

The register number 9 is encoded as `Rs`, but the invalid X-register class is not rejected.

## Impact

The assembler accepts invalid STXP/STLXP syntax and emits an instruction word for an operand form the ISA does not allow.

## Suggested fix

Require the status register to be W-class:

```rust
let rs_name = get_reg_name(operands, 0)?;
if !rs_name.starts_with('w') && !rs_name.starts_with('W') {
    return Err("stxp/stlxp status register must be W".to_string());
}
let rs = parse_reg_num(&rs_name).ok_or("invalid status register")?;
```

## Regression property

Failing property: `stxp_status_register_must_be_w`

```rust
prop_assert!(encode_ldxp_stxp(&[xreg(9), xreg(0), xreg(1), mem(xreg(2))], false).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/52
