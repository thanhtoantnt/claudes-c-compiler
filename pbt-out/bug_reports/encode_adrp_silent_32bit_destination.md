# Bug Report: `encode_adrp` silently accepts a 32-bit (W) destination register

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_adrp`
**Severity:** Medium

## Summary

ADRP materialises a 64-bit page-aligned address. Its opcode bit 31 is the fixed
`1` of `1 immlo 10000 immhi Rd`; there is **no** 32-bit form and no `sf` bit. Per
the ARMv8-A ARM (C6.2.10 "ADRP") the destination is `<Xd>` only. Both GAS and
LLVM-MC reject `adrp w0, sym` ("invalid operand for instruction" / "operand
mismatch -- register `w0' expected").

`encode_adrp` calls `get_reg(operands, 0)` and **discards** the returned `is_64`
flag, so `adrp w0, sym` returns `Ok` with a word byte-identical to
`adrp x0, sym` (`0x9000_0000`). The destination width is never validated.

## Root Cause

```rust
pub(crate) fn encode_adrp(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // <-- is_64 discarded; W accepted as X
    ...
}
```

## Reproduction

**Input:** `adrp w0, sym`  →  `encode_adrp(&[Operand::Reg("w0"), Operand::Symbol("A")])`

**Expected:** `Err` (ADRP is 64-bit-only; destination must be `<Xd>`).

**Actual:**
```
Ok(WordWithReloc { word: 2415919104 /* 0x9000_0000 */,
       reloc: Relocation { reloc_type: AdrpPage21, symbol: "A", addend: 0 } })
```
— byte-identical to `adrp x0, sym`.

**Minimal failing input:** `n = 0, sym = "A"` (i.e. `adrp w0, A`).

## Impact

A 32-bit destination operand to a 64-bit-only instruction is silently accepted
and assembled as the 64-bit form. Downstream the produced relocation + word will
write into the wrong width register without any error, masking an upstream
codegen/IR bug. The defect is invisible to the caller because the word looks
valid.

## Suggested Fix

Validate the width right after reading the register:
```rust
let (rd, is_64) = get_reg(operands, 0)?;
if !is_64 {
    return Err("adrp requires a 64-bit destination register (Xd)".to_string());
}
```

## Regression Property

Failing property: `prop_adrp_rejects_32bit_register`
(in `src/backend/arm/assembler/encoder/prop_adrp_cbz_tbz.rs`, `#[ignore]`d so the
default `cargo test` stays green; reproduce with
`cargo test --lib -- --ignored prop_adrp_rejects_32bit_register`).

```rust
#[test]
#[ignore = "documented bug: encode_adrp silently accepts a 32-bit W destination (ADRP is Xd-only)"]
fn prop_adrp_rejects_32bit_register(n in 0u32..=30u32, sym in arb_sym()) {
    let ops = vec![Operand::Reg(format!("w{}", n)), Operand::Symbol(sym)];
    prop_assert!(encode_adrp(&ops).is_err());
}
```

Minimal failing input today: `n = 0, sym = "A"`.
