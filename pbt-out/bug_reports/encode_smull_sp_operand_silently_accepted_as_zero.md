# Bug Report: `encode_smull` silently accepts SP as accumulator

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smull`
**Severity:** High

## Summary

`parse_reg_num` maps both `sp`/`wsp` and `xzr`/`wzr` to register number 31. SMULL encodes as `SMADDL Xd, Wn, Wm, XZR`, where `Ra = 31` means XZR, not SP. But `smull x0, w1, w2` with SP as implicit accumulator is accepted and produces wrong encoding.

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;  // sp → 31, treated as XZR
let (rn, _) = get_reg(operands, 1)?;
let (rm, _) = get_reg(operands, 2)?;
```

No SP-form validation.

## Reproduction

**Input:** `smull sp, w1, w2`

**Expected:** `Err` — SMULL destination must not be SP

**Actual:** `Ok(Word(...))` — SP encoded as register 31 (same as XZR)

## Impact

SP silently accepted and encoded as XZR. Wrong destination in multiply-accumulate path.

## Suggested Fix

Reject SP as destination:

```rust
let name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
if name == "sp" || name == "wsp" {
    return Err("SMULL destination must not be SP".into());
}
```

## Regression Property

Failing property: `smull_rejects_sp_destination`

```rust
prop_assert!(encode_smull(&[Operand::Reg("sp".into()), wreg(1), wreg(2)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/98