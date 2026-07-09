# Bug Report: `encode_neon_mla` silently accepts mismatched arrangement operands

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mla`
**Severity:** Medium

## Summary

`encode_neon_mla` discards the arrangement specifiers of the `Vn` and `Vm`
operands (bound to `_`) and derives `Q`/`size` solely from the destination
arrangement. As a result, instructions whose three arrangement specifiers do
**not** match — e.g. `mla v0.4s, v1.8b, v2.2s` — are silently accepted and
encoded as if all three had the destination arrangement, instead of being
rejected. The authoritative assembler `llvm-mc-18 --triple=aarch64` rejects
these with `error: invalid operand for instruction`.

This is **distinct** from the already-reported `encode_neon_mla_unallocated_doubleword`
defect (which is about an *unallocated* `size=0b11` field); here all three
arrangements are individually *valid*, they are simply *inconsistent*.

## Root Cause

```rust
pub(crate) fn encode_neon_mla(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;   // arr_n IGNORED
    let (rm, _) = get_neon_reg(operands, 2)?;   // arr_m IGNORED
    let (q, size) = neon_arr_to_q_size(&arr_d)?;
    ...
}
```

There is no cross-operand consistency check that `arr_d == arr_n == arr_m`.

## Reproduction

**Input:** `mla v0.4s, v1.8b, v2.2s`

**Expected:** `Err` — MLA requires all three arrangements to match
(`llvm-mc-18`: `error: invalid operand for instruction`).

**Actual:** `Ok(EncodeResult::Word(0x4EA29420))` — silently coerced to the
destination `.4s` arrangement (identical to `mla v0.4s, v1.4s, v2.4s`).

**Minimal failing input:** `rd=0, rn=1, rm=2`, arrangements `("4s","8b","2s")`.

## Impact

Silent mis-assembly: an inconsistent operand list is accepted and turned into
the encoding of the (presumed) matching form, masking programmer error. A
correct assembler rejects this at assembly time.

## Suggested Fix

Validate that all three arrangement specifiers agree:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let (rm, arr_m) = get_neon_reg(operands, 2)?;
if arr_n != arr_d || arr_m != arr_d {
    return Err(format!(
        "MLA requires matching arrangements; got .{arr_d}/.{arr_n}/.{arr_m}"
    ));
}
let (q, size) = neon_arr_to_q_size(&arr_d)?;
```

## Regression test

`#[ignore]`d witness in
`src/backend/arm/assembler/encoder/neon_mla_mls_fields_pbt.rs`:
`mla_rejects_mismatched_arrangement`.

Reproduce (fails today, passes once fixed):

```
cargo test --lib mla_rejects_mismatched_arrangement -- --ignored
```

## Verification of the (correct) field encoding

For reference, the register-field placement, opcode (`10010`), and `U`-bit
(`0`) were verified **correct** for valid inputs via a differential oracle
against `llvm-mc-18`. The mismatched-arrangement acceptance above is the
genuine defect; the opcode/u-bit/register placement are not.
