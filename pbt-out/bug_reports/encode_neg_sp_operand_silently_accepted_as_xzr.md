# Bug Report: `encode_neg` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_neg`
**Severity:** High

## Summary

`encode_neg` (`NEG <Rd>,<Rm>` = alias of `SUB <Rd>,XZR,<Rm>`) resolves operands through the shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number **31**. The add/subtract **(shifted register)** group reads operand field `31` as **XZR/WZR**, *not* SP. NEG has **no** SP-using variant (only ADD/SUB immediate and extended-register forms accept SP). Per the ARMv8 ARM, supplying SP as `Rd` or `Rm` of NEG is **unallocated**.

Consequently `neg sp, x0` is **silently** encoded as `neg xzr, x0`, and `neg x0, sp` as `neg x0, xzr` (i.e. `x0 := 0`), returning `Ok(Word(...))` with no diagnostic. The bug applies to both operand positions.

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

**Input:** `neg x0, sp`

**Expected:** `Err` — SP not permitted in NEG operands

**Actual:** `Ok(Word(...))` — bit-identical to `neg x0, xzr` (destination := 0)

**Minimal failing input:** `encode_neg(&[Operand::Reg("x0".into()), Operand::Reg("sp".into())])`

Differential check: `echo 'neg x0, sp' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `neg ..., sp` is assembled as if the source were the zero register, zeroing the result instead of negating the stack pointer, with no assembler error.

## Suggested Fix

Reject SP/WSP in `encode_neg` before encoding (both operand positions), using the shared `reject_sp` helper.

## Regression Property

Failing property: `neg_rejects_sp_wsp_in_any_position`

```rust
// cargo test --lib data_processing_adc_sbc_neg_negs_pbt::neg_rejects_sp_wsp_in_any_position -- --ignored
prop_assert!(encode_neg(&[Operand::Reg("x0".into()), Operand::Reg("sp".into())]).is_err());
```


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/287
