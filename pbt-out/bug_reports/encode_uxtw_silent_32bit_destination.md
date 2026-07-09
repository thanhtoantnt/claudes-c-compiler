# Bug Report: `encode_uxtw` silently accepts the invalid 32-bit form `uxtw Wd, Wn`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_uxtw`
**Severity:** Medium

## Summary

`encode_uxtw` silently accepts the 32-bit destination form `uxtw Wd, Wn`, which is
not a valid AArch64 instruction and is rejected by the reference assembler
(`llvm-mc-18`). `UXTW` has no standalone `UXTB`/`UXTH`-style alias for a 32-bit
destination in the ARMv8 ARM — the only valid standalone spelling is the 64-bit
form `uxtw Xd, Wn`. A conforming assembler must return `Err` for `uxtw Wd, Wn`.

## Root Cause

```rust
pub(crate) fn encode_uxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    // UXTW is MOV Wd, Wn (the upper 32 bits are zeroed)
    // Or: UBFM Xd, Xn, #0, #31
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    // Use 32-bit ORR (MOV alias)
    let word = (0b001010100 << 23) | (rn << 16) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

Both `get_reg` width flags are bound to `_` and ignored, so the encoder never
checks that the destination is 64-bit. The destination width being discarded also
means `uxtw x0, w1`, `uxtw w0, w1`, and `uxtw x0, x1` all produce the identical
word — operand widths are never validated at all.

## Reproduction

Differential check against `llvm-mc-18 --triple=aarch64`:

```
$ echo 'uxtw w0, w1' | llvm-mc-18 --triple=aarch64
<stdin>:1:6: error: invalid operand for instruction
```

- **input:** `uxtw w0, w1` → `ops = [Reg("w0"), Reg("w1")]`
- **actual:** `Ok(EncodeResult::Word(0x2A0103E0))` (accepted, silently assembled
  into `mov w0, w1`)
- **expected:** `Err`

## Impact

An architecturally invalid mnemonic is silently assembled into a different
instruction, defeating assembler validation. A user writing `uxtw w0, w1` (a typo
or misconception) gets no diagnostic and instead emits `mov w0, w1`, which can
mask downstream bugs. The lack of any width check also makes `uxtw x0, x1`
(source wrongly 64-bit) succeed, masking another class of input errors.

## Suggested Fix

Validate that the destination is a 64-bit `Xd` and reject the 32-bit form (the
companion fix to emitting the canonical `UBFM` form, see
`encode_uxtw_emits_mov_not_ubfm.md`):

```rust
pub(crate) fn encode_uxtw(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, rd_is_64) = get_reg(operands, 0)?;
    let (rn, rn_is_64) = get_reg(operands, 1)?;
    // UXTW is only valid as `uxtw Xd, Wn` (UBFM Xd, Xn, #0, #31).
    if !rd_is_64 || rn_is_64 {
        return Err("uxtw requires <Xd>, <Wn>".to_string());
    }
    let word = 0xD3407C00u32 | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Regression Property

Failing property: `uxtw_rejects_32bit_destination_form` (mod `uxtw_props`,
`data_processing.rs:7618`).

```rust
#[test]
fn uxtw_rejects_32bit_destination_form() {
    let ops = vec![wreg(0), wreg(1)]; // uxtw w0, w1 -- invalid
    assert!(encode_uxtw(&ops).is_err()); // llvm-mc rejects; encoder must too
}
```

**Reproduce:** `cargo test --lib data_processing::uxtw_props::uxtw_rejects_32bit_destination_form`
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/203
