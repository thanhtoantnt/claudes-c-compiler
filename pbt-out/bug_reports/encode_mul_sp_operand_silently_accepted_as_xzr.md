# Bug Report: `encode_mul` silently accepts SP in any operand (encodes as multiply-by-zero)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_mul`
**Severity:** High

## Summary

`encode_mul` uses the shared `get_reg` helper to resolve register operands. `get_reg` maps the names `"sp"` and `"wsp"` to register number **31**. In the ARMv8 **MADD/MUL** encoding (`sf 0 0 11011 000 Rm 0 Ra Rn Rd`), operand field value `31` is decoded as **XZR (zero register)**, *not* SP. MADD/MUL does **not** have an SP-using variant (unlike ADD/SUB). Per the ARMv8 ARM, supplying SP in any of Rd/Rn/Rm/Ra of a data-processing register operation is **UNPREDICTABLE / unallocated**.

Consequently `mul x0, x1, sp` is **silently** encoded as `mul x0, x1, xzr` — i.e. a multiply-by-zero — with `Ok(Word(...))` and no diagnostic.

## Root Cause

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    match name.as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr => Some(31),
        ...
```

The shared helper maps `sp`/`wsp` → 31 unconditionally, but register field 31 in MUL means XZR, not SP.

## Reproduction

**Input:** `mul x0, x1, sp`

**Expected:** `Err` — SP not permitted in MUL/MADD operands

**Actual:** `Ok(Word(0x9B217C20))` → `mul x0, x1, xzr` (multiply-by-zero)

**Minimal failing input:** `encode_mul(&[xreg(0), xreg(1), Operand::Reg("sp".into())])`

## Impact

**Silent mis-compilation**: A program containing `mul x0, x1, sp` gets assembled to an instruction that always yields 0 instead of the product, with no assembler error. The same issue affects Rd and Rn positions. This is a codebase-wide pattern affecting `encode_madd`, `encode_msub`, `encode_mneg`, `encode_div`, `encode_mulh`, `encode_smulh`, `encode_logical`, `encode_shift`, etc.

## Suggested Fix

Reject SP/WSP in `encode_mul` before encoding:

```rust
fn reject_sp(operands: &[Operand]) -> Result<(), String> {
    for op in operands {
        if let Operand::Reg(r) = op {
            if r.eq_ignore_ascii_case("sp") || r.eq_ignore_ascii_case("wsp") {
                return Err("MUL: SP/WSP is not a valid operand".into());
            }
        }
    }
    Ok(())
}
```

## Regression Property

Failing property: `mul_sp_in_rm_is_silently_accepted_as_xzr`

```rust
prop_assert!(encode_mul(&[xreg(0), xreg(1), Operand::Reg("sp".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/71