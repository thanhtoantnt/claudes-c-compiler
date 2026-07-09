# Bug Report: `encode_umull` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umull`
**Severity:** High

## Summary

`encode_umull` (`UMULL <Xd>,<Wn>,<Wm>` = alias of `UMADDL <Xd>,<Wn>,<Wm>,XZR`)
resolves operands through the shared `get_reg`/`parse_reg_num` helpers, which map
`"sp"`/`"wsp"` → register number **31**. In the widening-multiply (3 source)
group (`1 00 11011 101 Rm 0 Ra Rn Rd`), operand field value `31` is decoded as
**XZR/WZR (zero register)**, *not* SP. UMULL has **no** SP-using variant. Per the
ARMv8 ARM, supplying SP in any of `Xd`/`Wn`/`Wm` is **unallocated**.

Consequently `umull x0, w0, sp` is **silently** encoded as `umull x0, w0, wzr`
(unsigned multiply by zero), returning `Ok(Word(...))` with no diagnostic.

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

**Input:** `umull x0, w0, sp`
**Expected:** `Err` — SP not permitted in UMULL operands
**Actual:** `Ok(Word(...))` — bit-identical to `umull x0, w0, wzr`
**Minimal failing input:** `encode_umull(&[Operand::Reg("x0".into()), Operand::Reg("w0".into()), Operand::Reg("sp".into())])`

Differential check: `echo 'umull x0, w0, sp' | clang --target=aarch64-linux-gnu -c -x assembler -` → `error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `umull ..., sp` is assembled as if the source were the
zero register (forcing a zero result), with no assembler error.

## Suggested Fix

Reject SP/WSP in `encode_umull` before encoding (all operand positions), using
the shared `reject_sp` helper.

## Regression Property

Failing witness: `umull_rejects_sp_wsp_in_any_position`

```text
cargo test --lib data_processing_mul_madd_msub_umaddl_umull_pbt::umull_rejects_sp_wsp_in_any_position -- --ignored
```

**GitHub Issue:** (none)
