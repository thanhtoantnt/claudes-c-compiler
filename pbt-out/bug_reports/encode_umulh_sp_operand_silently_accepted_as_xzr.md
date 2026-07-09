# Bug Report: `encode_umulh` silently accepts SP/WSP as destination (encodes as XZR)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_umulh`
**Severity:** High

## Summary

`encode_umulh` (`UMULH <Xd>,<Xn>,<Xm>`, 64-bit-only) resolves operands through
the shared `get_reg`/`parse_reg_num` helpers, which map `"sp"`/`"wsp"` →
register number **31**. In the data-processing (3 source) group
(`1 00 11011 110 Rm 0 11111 Rn Rd`), operand field value `31` is decoded as
**XZR/WZR (zero register)**, *not* SP. UMULH has **no** SP-using variant. Per
the ARMv8 ARM, supplying SP/WSP in any of `Xd`/`Xn`/`Xm` is **unallocated**.

Consequently `umulh sp, x0, x0` is **silently** encoded as
`umulh xzr, x0, x0` (writing the result to the zero register, discarding it),
returning `Ok(Word(...))` with no diagnostic.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    match name.to_lowercase().as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr" => Some(31),
        ...
```

Register field 31 means XZR/WZR in this encoding group, so `sp`/`wsp` are
indistinguishable from `xzr`/`wzr`. UMULH never inspects width or register class.

## Reproduction

**Input:** `umulh sp, x0, x0`
**Expected:** `Err` — SP/WSP not permitted in UMULH operands
**Actual:** `Ok(Word(2613083167))` — bit-identical to `umulh xzr, x0, x0`
**Minimal failing input (PBT-shrunk):** `encode_umulh(&[Operand::Reg("sp".into()), xreg(0), xreg(0)])`

Differential oracle:
```text
$ echo 'umulh sp, x1, x2' | clang --target=aarch64-linux-gnu -c -x assembler -
<stdin>:1:7: error: invalid operand for instruction
```

## Impact

Silent mis-compilation: a destination of `sp`/`wsp` is assembled as if the
destination were the zero register — the high-half multiply result is silently
discarded, with no assembler error. Same class of defect as the existing
`encode_smulh` SP report; UMULH is in the identical encoding group (only op31
differs).

## Suggested Fix

Reject SP/WSP in `encode_umulh` before encoding (all three operand positions),
using the shared `reject_sp` helper (mirroring the fix prescribed for SMULH).

## Regression Property

Failing witness: `wit_umulh_rejects_sp_as_destination`

```text
cargo test --lib data_processing_smulh_umulh_uxtb_uxth_pbt::wit_umulh_rejects_sp_as_destination -- --ignored
```

**GitHub Issue:** (none)
