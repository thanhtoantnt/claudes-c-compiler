# Bug Report: `encode_neon_mla` emits unallocated doubleword encoding

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_mla`

## Summary

`encode_neon_mla` accepts the `.1d` arrangement (element size `size = 0b11`)
and emits a 32-bit instruction word, but that encoding is **UNALLOCATED** for
the `MLA` (vector) instruction. It should return `Err`.

The ARMv8-A ARM (ARM DDI 0487, "Advanced SIMD three same") defines `MLA`
(vector) only for the integer multiply element sizes
(`T = 8B, 16B, 4H, 8H, 2S, 4S`); the `size == 0b11` row is UNALLOCATED, so `MLA`
performs no doubleword multiply.

## Root cause

```rust
pub(crate) fn encode_neon_mla(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;   // <-- accepts .1d (size=0b11)
    // MLA: 0 Q 0 01110 size 1 Rm 10010 1 Rn Rd
    let word = (q << 30) | (0b001110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b100101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`neon_arr_to_q_size` maps `1d`→`(0, 0b11)`, so `size = 0b11` flows straight into
the word. There is no guard rejecting the unallocated element size.

## Minimal input

```
mla v0.1d, v1.1d, v2.1d
```

- **Expected:** `Err` (`.1d` ⇒ `size = 0b11` is UNALLOCATED for MLA)
- **Actual:** `Ok(EncodeResult::Word(0x0EE29420))` — `size` field (bits 23-22) = `0b11`
- **Reproduce:** `cargo test --lib -- --ignored neon_mla_pbt::mla_rejects_doubleword`
  (fails: `expected Err but got Ok(0x0EE29420)`)

## Impact

A NEON `MLA` written with the `.1d` arrangement silently assembles into an
UNALLOCATED instruction word. No conforming AArch64 core decodes it as
multiply-accumulate — it raises an exception at runtime. The defect is silent:
the assembler returns `Ok`, so it cannot be caught without an external reference
assembler.

## Suggested fix

Reject `size == 0b11` before building the word:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 {
    return Err(format!("mla: unsupported arrangement {} (no doubleword multiply)", arr_d));
}
```

## Validation

Properties in `src/backend/arm/assembler/encoder/neon_mla_pbt.rs`
(`cargo test --lib neon_mla_pbt`): 5 passing properties plus the
`#[ignore]`d `mla_rejects_doubleword` reproducer, which fails as shown above
(confirmed: `expected Err but got Ok(0x0EE29420)`).
