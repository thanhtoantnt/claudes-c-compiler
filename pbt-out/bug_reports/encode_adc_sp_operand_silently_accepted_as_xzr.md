# Bug Report: `encode_adc` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_adc`
**Severity:** High

## Summary

`encode_adc` resolves operands through the shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number **31**. In the ARMv8 **add/subtract (with carry)** group (`sf op S 11010000 Rm 000000 Rn Rd`), operand field value `31` is decoded as **XZR/WZR (zero register)**, *not* SP. ADC/ADCS has **no** SP-using variant. Per the ARMv8 ARM, supplying SP in any of Rd/Rn/Rm is **unallocated**.

Consequently `adc x0, sp, x1` is **silently** encoded as `adc x0, xzr, x1` (carry-add of zero instead of the stack pointer), returning `Ok(Word(...))` with no diagnostic. The bug applies to all three operand positions and to both ADC and ADCS.

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

**Input:** `adc x0, sp, x1`

**Expected:** `Err` — SP not permitted in ADC operands

**Actual:** `Ok(Word(...))` — bit-identical to `adc x0, xzr, x1`

**Minimal failing input:** `encode_adc(&[Operand::Reg("x0".into()), Operand::Reg("sp".into()), Operand::Reg("x1".into())], false)`

Differential check: `echo 'adc x0, sp, x1' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a program using `adc ..., sp, ...` is assembled to an instruction that adds the zero register (ignoring the actual stack pointer), with no assembler error. Any dependence on SP in a multi-word add is silently broken.

## Suggested Fix

Reject SP/WSP in `encode_adc` before encoding (all operand positions):

```rust
fn reject_sp(operands: &[Operand]) -> Result<(), String> {
    for op in operands {
        if let Operand::Reg(r) = op {
            let r = r.to_lowercase();
            if r == "sp" || r == "wsp" {
                return Err("ADC: SP/WSP is not a valid operand".into());
            }
        }
    }
    Ok(())
}
```

## Regression Property

Failing property: `adc_rejects_sp_wsp_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::adc_rejects_sp_wsp_in_any_position -- --ignored
prop_assert!(encode_adc(&[Operand::Reg("x0".into()), Operand::Reg("sp".into()), Operand::Reg("x1".into())], false).is_err());
```

**GitHub Issue:** (none)
