# Bug Report: `encode_ret` silently accepts SP as register target

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_ret`
**Severity:** High

## Summary

`parse_reg_num` maps both `sp` and `xzr` to register number 31. RET encoding reserves `Xn = 31` for `XZR`. `ret sp` accepted and encoded as `ret xzr`, returning to address 0 instead of error.

## Root Cause

```rust
let (rn, _) = get_reg(operands, 0)?;   // sp → 31, treated as XZR
```

No SP-form validation in `encode_ret`.

## Reproduction

**Input:** `ret sp`

**Expected:** `Err` — RET target must not be SP (use XZR for return to zero)

**Actual:** `Ok(Word(0xD65F0000))` — encoded as `ret xzr` (sp silently coerced)

## Impact

SP operand silently replaced by XZR, causing return to address 0 instead of using SP. No diagnostic, silently wrong control flow.

## Suggested Fix

Reject SP/WSP for return target:

```rust
let (rn, _) = get_reg(operands, 0)?;
let name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
if name == "sp" || name == "wsp" {
    return Err("RET target must not be SP (use XZR/WZR for return to zero)".into());
}
```

## Regression Property

Failing property: `ret_rejects_sp_operand`

```rust
prop_assert!(encode_ret(&[Operand::Reg("sp".into())]).is_err());
prop_assert!(encode_ret(&[Operand::Reg("wsp".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/106