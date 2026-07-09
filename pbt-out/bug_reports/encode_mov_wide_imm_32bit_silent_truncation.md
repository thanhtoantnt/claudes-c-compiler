# `encode_mov_wide_imm` silently truncates 32-bit immediates beyond `0xFFFFFFFF`

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` →
`pub(crate) fn encode_mov_wide_imm(rd: u32, is_64: bool, imm: u64)`
**Witness property:** `mov_wide_rejects_32bit_overflow`
(`#[ignore]`d, expected to fail)
**File:** `src/backend/arm/assembler/encoder/data_processing_mov_wide_imm_pbt.rs`
**Reproduce:** `cargo test --lib -- --ignored mov_wide_rejects_32bit_overflow`

## Summary

When `is_64 == false`, `encode_mov_wide_imm` walks only `hw ∈ {0, 1}` and emits a
`MOVZ`/`MOVK` sequence for the **low 32 bits** of `imm`, silently discarding any
bits above bit 31. The ARMv8-A ARM restricts a `MOVZ`/`MOVK` with `sf = 0`
(32-bit destination) to `hw ∈ {0, 1}` and to a 32-bit immediate; there is no
legal encoding that materialises a value with bits above bit 31 into a `Wd`
register. A real assembler rejects the input instead of dropping the high half:

```
$ echo 'mov w0, #0x100000001' | llvm-mc-18 --triple=aarch64 -show-encoding
error: immediate must be an integer in range [0, 4294967295]
```

## Root cause

```rust
let max_hw = if is_64 { 4 } else { 2 };
for hw in 0..max_hw {
    let chunk = ((imm >> (hw * 16)) & 0xFFFF) as u32;   // only bits 0..31 read
    ...
}
```

There is no check that `imm` fits in the operand width before emitting.

## Falsifiable / minimal failing input

`rd = 0, is_64 = false, hi = 1, lo = 0` → `imm = 0x1_0000_0000`.

- **property:** `mov_wide_rejects_32bit_overflow`
- **actual:** `Ok(EncodeResult::Word(...))` — a single `MOVZ w0, #0` that
  materialises `0x00000000`, dropping the high half of the requested
  `0x1_0000_0000`.
- **expected:** `Err` (immediate must be `<= 0xFFFF_FFFF` for a 32-bit dest).

## Impact

Any caller reaching `encode_mov_wide_imm` with a 32-bit destination and an
immediate larger than `0xFFFF_FFFF` (e.g. a negative `i64` widened via
`imm as u64` in the `mov Xd, #imm` path of `encode_mov`) produces machine code
that materialises a **different** value than the source requested — a silent
miscompilation with no diagnostic.

## Suggested fix

```rust
if !is_64 && imm > 0xFFFF_FFFF {
    return Err(format!("32-bit mov immediate 0x{:x} exceeds 32 bits", imm));
}
```
