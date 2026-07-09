# Bug Report: `encode_smaddl` silently accepts SP/WSP in any operand (encodes as XZR/WZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_smaddl`
**Severity:** High

## Summary

`encode_smaddl` (`SMADDL <Xd>,<Wn>,<Wm>,<Xa>`) resolves operands through the
shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` → register
number **31**. In the widening-multiply (3 source) group
(`1 00 11011 001 Rm 0 Ra Rn Rd`), operand field value `31` is decoded as
**XZR/WZR (zero register)**, *not* SP. SMADDL has **no** SP-using variant. Per
the ARMv8 ARM, supplying SP/WSP in any of `Xd`/`Wn`/`Wm`/`Xa` is **unallocated**.

Consequently `smaddl x0, w1, w2, sp` is **silently** encoded as
`smaddl x0, w1, w2, xzr` (accumulate zero), returning `Ok(Word(...))` with no
diagnostic.

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

**Input:** `smaddl x0, w1, w2, sp`
**Expected:** `Err` — SP/WSP not permitted in SMADDL operands
**Actual:** `Ok(Word(...))` — bit-identical to `smaddl x0, w1, w2, xzr`
**Minimal failing input (PBT-shrunk):** `encode_smaddl(&[Operand::Reg("sp".into()), wreg(0), wreg(0), xreg(0)])`

Differential oracle: `echo 'smaddl x0, w1, w2, sp' | clang --target=aarch64-linux-gnu -c -x assembler -` →
`error: invalid operand for instruction`.

## Impact

Silent mis-compilation: `smaddl ..., sp` is assembled as if the accumulator were
the zero register (forcing `Xd = Wn*Wm + 0`), with no assembler error.

## Suggested Fix

Reject SP/WSP in `encode_smaddl` before encoding (all four operand positions),
using the shared `reject_sp` helper.

## Regression Property

Failing witness: `smaddl_rejects_sp_wsp_in_any_position`

```text
cargo test --lib data_processing_smull_smaddl_smulh_mneg_fpsimd_sp_pbt::smaddl_rejects_sp_wsp_in_any_position -- --ignored
```

**GitHub Issue:** (none)
