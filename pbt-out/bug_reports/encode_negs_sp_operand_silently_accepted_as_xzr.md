# Bug Report: `encode_negs` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_negs`
**Severity:** High

## Summary

`encode_negs` (`NEGS <Rd>,<Rm>` = alias of `SUBS <Rd>,XZR,<Rm>`) resolves operands through the shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number **31**. The add/subtract **(shifted register)** group reads operand field `31` as **XZR/WZR**, *not* SP. NEGS has **no** SP-using variant. Per the ARMv8 ARM, supplying SP as `Rd` or `Rm` of NEGS is **unallocated**.

Consequently `negs sp, x0` is **silently** encoded as `negs xzr, x0`, and `negs x0, sp` as `negs x0, xzr` (destination := 0 and flags as if subtracting zero), returning `Ok(Word(...))` with no diagnostic. The bug applies to both operand positions.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    match name.to_lowercase().as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr" => Some(31),
        ...
```

Register field 31 means XZR/WZR in this encoding group, so `sp`/`wsp` are indistinguishable from `xzr`/`wzr`.

## Reproduction

**Input:** `negs x0, sp`

**Expected:** `Err` — SP not permitted in NEGS operands

**Actual:** `Ok(Word(...))` — bit-identical to `negs x0, xzr`

**Minimal failing input:** `encode_negs(&[Operand::Reg("x0".into()), Operand::Reg("sp".into())])`

Differential check: `echo 'negs x0, sp' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `negs ..., sp` is assembled as if the source were the zero register, with no assembler error; the destination and condition flags are computed from the wrong value.

## Suggested Fix

Reject SP/WSP in `encode_negs` before encoding (both operand positions), using the shared `reject_sp` helper.

## Regression Property

Failing property: `negs_rejects_sp_wsp_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::negs_rejects_sp_wsp_in_any_position -- --ignored
prop_assert!(encode_negs(&[Operand::Reg("x0".into()), Operand::Reg("sp".into())]).is_err());
```

**GitHub Issue:** (none)
