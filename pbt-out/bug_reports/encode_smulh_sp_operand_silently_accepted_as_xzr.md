# Bug Report: `encode_smulh` silently accepts SP/WSP in any operand (encodes as XZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smulh`
**Severity:** High

## Summary

`encode_smulh` (`SMULH <Xd>,<Xn>,<Xm>`) resolves operands through the shared
`get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register number
**31**. In the data-processing (3 source) group
(`1 00 11011 010 Rm 0 11111 Rn Rd`), operand field value `31` is decoded as
**XZR/WZR (zero register)**, *not* SP. SMULH has **no** SP-using variant. Per the
ARMv8 ARM, supplying SP/WSP in any of `Xd`/`Xn`/`Xm` is **unallocated**.

Consequently `smulh x0, sp, x2` is **silently** encoded as `smulh x0, xzr, x2`
(signed multiply by zero), returning `Ok(Word(...))` with no diagnostic.

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

**Input:** `smulh x0, sp, x2`
**Expected:** `Err` — SP/WSP not permitted in SMULH operands
**Actual:** `Ok(Word(...))` — bit-identical to `smulh x0, xzr, x2`
**Minimal failing input (PBT-shrunk):** `encode_smulh(&[Operand::Reg("sp".into()), xreg(0), xreg(0)])`

Differential oracle: `echo 'smulh x0, sp, x2' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `smulh ..., sp` is assembled as if the source were the
zero register (forcing a zero result), with no assembler error.

## Suggested Fix

Reject SP/WSP in `encode_smulh` before encoding (all three operand positions),
using the shared `reject_sp` helper.

## Regression Property

Failing witness: `smulh_rejects_sp_wsp_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::smulh_rejects_sp_wsp_in_any_position -- --ignored
```

**GitHub Issue:** (none)

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/338
