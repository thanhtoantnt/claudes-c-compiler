# Bug Report: `encode_sbc` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sbc`
**Severity:** High

## Summary

`encode_sbc` resolves operands through the shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number **31**. In the ARMv8 **add/subtract (with carry)** group (`sf 1 S 11010000 Rm 000000 Rn Rd`), operand field value `31` is decoded as **XZR/WZR (zero register)**, *not* SP. SBC/SBCS has **no** SP-using variant. Per the ARMv8 ARM, supplying SP in any of Rd/Rn/Rm is **unallocated**.

Consequently `sbc x0, sp, x1` is **silently** encoded as `sbc x0, xzr, x1` (carry-subtract of zero instead of the stack pointer), returning `Ok(Word(...))` with no diagnostic. The bug applies to all three operand positions and to both SBC and SBCS.

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

**Input:** `sbc x0, sp, x1`

**Expected:** `Err` — SP not permitted in SBC operands

**Actual:** `Ok(Word(...))` — bit-identical to `sbc x0, xzr, x1`

**Minimal failing input:** `encode_sbc(&[Operand::Reg("x0".into()), Operand::Reg("sp".into()), Operand::Reg("x1".into())], false)`

Differential check: `echo 'sbc x0, sp, x1' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a program using `sbc ..., sp, ...` is assembled to an instruction that subtracts the zero register (ignoring the actual stack pointer), with no assembler error. Any multi-word subtraction that depends on SP is silently broken.

## Suggested Fix

Reject SP/WSP in `encode_sbc` before encoding (all operand positions), as shown for `encode_adc` (`reject_sp` helper).

## Regression Property

Failing property: `sbc_rejects_sp_wsp_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::sbc_rejects_sp_wsp_in_any_position -- --ignored
prop_assert!(encode_sbc(&[Operand::Reg("x0".into()), Operand::Reg("sp".into()), Operand::Reg("x1".into())], false).is_err());
```

**GitHub Issue:** (none)
