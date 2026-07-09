# Bug Report: `encode_mneg` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mneg`
**Severity:** High

## Summary

`encode_mneg` (`MNEG <Rd>,<Rn>,<Rm>` = alias of `MSUB Rd,Rn,Rm,XZR`) resolves
operands through the shared `get_reg`/`parse_reg_num` helpers, which map
`"sp"`/`"wsp"` → register number **31**. In the data-processing (3 source) group
(`sf 00 11011 000 Rm 1 11111 Rn Rd`), operand field value `31` is decoded as
**XZR/WZR (zero register)**, *not* SP. MNEG has **no** SP-using variant. Per the
ARMv8 ARM, supplying SP/WSP in any of `Rd`/`Rn`/`Rm` is **unallocated**.

Consequently `mneg x0, sp, x2` is **silently** encoded as `mneg x0, xzr, x2`
(negate of zero), returning `Ok(Word(...))` with no diagnostic.

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

**Input:** `mneg x0, sp, x2`
**Expected:** `Err` — SP/WSP not permitted in MNEG operands
**Actual:** `Ok(Word(...))` — bit-identical to `mneg x0, xzr, x2`
**Minimal failing input (PBT-shrunk):** `encode_mneg(&[Operand::Reg("sp".into()), xreg(0), xreg(0)])`

Differential oracle: `echo 'mneg x0, sp, x2' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `mneg ..., sp` is assembled as if the source were the
zero register (forcing a zero/negated result), with no assembler error.

## Suggested Fix

Reject SP/WSP in `encode_mneg` before encoding (all three operand positions),
using the shared `reject_sp` helper.

## Regression Property

Failing witness: `mneg_rejects_sp_wsp_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::mneg_rejects_sp_wsp_in_any_position -- --ignored
```

**GitHub Issue:** (none)
