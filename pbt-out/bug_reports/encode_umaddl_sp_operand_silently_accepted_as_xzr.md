# Bug Report: `encode_umaddl` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umaddl`
**Severity:** High

## Summary

`encode_umaddl` (`UMADDL <Xd>,<Wn>,<Wm>,<Xa>`) resolves operands through the shared
`get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number
**31**. In the widening-multiply (3 source) group (`1 00 11011 101 Rm 0 Ra Rn Rd`),
operand field value `31` is decoded as **XZR/WZR (zero register)**, *not* SP.
UMADDL has **no** SP-using variant. Per the ARMv8 ARM, supplying SP in any of
`Xd`/`Wn`/`Wm`/`Xa` is **unallocated**.

Consequently `umaddl x0, w0, sp, x0` is **silently** encoded as
`umaddl x0, w0, wzr, x0`, returning `Ok(Word(...))` with no diagnostic.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    match name.to_lowercase().as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr" => Some(31),
        ...
```

Register field 31 means XZR/WZR in this encoding group, so `sp`/`wsp` are
indistinguishable from `xzr`/`wzr`.

## Reproduction

**Input:** `umaddl x0, w0, sp, x0`
**Expected:** `Err` — SP not permitted in UMADDL operands
**Actual:** `Ok(Word(...))` — bit-identical to `umaddl x0, w0, wzr, x0`
**Minimal failing input:** `encode_umaddl(&[Operand::Reg("x0".into()), Operand::Reg("w0".into()), Operand::Reg("sp".into()), Operand::Reg("x0".into())])`

Differential check: `echo 'umaddl x0, w0, sp, x0' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: a SP-named source/accumulator is read as the zero
register with no assembler error, corrupting the multiply-add result.

## Suggested Fix

Reject SP/WSP in `encode_umaddl` before encoding (all operand positions), using
the shared `reject_sp` helper.

## Regression Property

Failing witness: `umaddl_rejects_sp_wsp_in_any_position`

```text
cargo test --lib data_processing_mul_madd_msub_umaddl_umull_pbt::umaddl_rejects_sp_wsp_in_any_position -- --ignored
```

**GitHub Issue:** (none)
