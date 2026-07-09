# Bug Report: `encode_msub` silently accepts `sp` / `wsp` as accumulators

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_msub`
**Severity:** High

## Summary

`parse_reg_num` maps both `sp`/`wsp` and `xzr`/`wzr` to register number 31. MSUB encoding reserves `Ra = 31` for **XZR/WZR** (zero accumulator), not SP. `msub x0, x1, x2, sp` accepted and encoded as `msub x0, x1, x2, xzr`.

## Root Cause

```rust
let (ra, _) = get_reg(operands, 3)?;   // sp/wsp → 31, treated as XZR/WZR
```

No SP-form validation in `encode_msub`.

## Reproduction

**Input:** `msub x0, x1, x2, sp`

**Expected:** `Err` — MSUB Ra must not be SP (use XZR/WZR for zero accumulator)

**Actual:** `Ok(Word(...))` — encoded as `msub x0, x1, x2, xzr`

**Minimal failing input:** rd=0, rn=1, rm=2, ra="sp"

## Impact

SP operand silently replaced by XZR, producing multiply-subtract with zero accumulator instead of using SP. No diagnostic, silently wrong operation.

## Suggested Fix

Reject SP/WSP for Ra:

```rust
let (ra, _) = get_reg(operands, 3)?;
let ra_name = match &operands[3] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
if ra_name == "sp" || ra_name == "wsp" {
    return Err("MSUB Ra must not be SP (use XZR/WZR for zero accumulator)".into());
}
```

## Regression Property

Failing property: `msub_rejects_sp_operand`

```rust
prop_assert!(encode_msub(&[xreg(0), xreg(1), xreg(2)], sp()).is_err());
prop_assert!(encode_msub(&[wreg(0), wreg(1), wreg(2)], wsp()).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/69