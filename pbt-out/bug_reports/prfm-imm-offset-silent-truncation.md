# Bug: `encode_prfm` immediate form silently truncates out-of-range offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_prfm`, `Operand::Mem` arm

## Summary

The PRFM **immediate** (unsigned-offset) form computes the scaled immediate as
`(imm / 8) as u32` **before** the `> 0xFFF` range check. When the scaled value
(`imm/8`) is `>= 2^32`, the truncating cast wraps to a small `u32`, the range
check sees the wrapped value and passes, and the encoder returns `Ok` for an
offset that is far out of range — silently mis-assembling it as if it were a tiny
offset instead of rejecting it with `Err`.

## Root cause

```rust
Operand::Mem { base, offset } => {
    let rn = parse_reg_num(base).ok_or_else(|| ...)?;
    let imm = *offset;
    if imm < 0 || imm % 8 != 0 { return Err(...); }
    let imm12 = (imm / 8) as u32;          // BUG: wraps for imm/8 >= 2^32
    if imm12 > 0xFFF { return Err(...); }  // check sees the wrapped value
    let word = 0xF9800000 | (imm12 << 10) | (rn << 5) | prfop;
    Ok(EncodeResult::Word(word))
}
```

## Observed consequences

For `prfm pldl1keep, [x0]` with `offset = 34359738368` (`imm/8 = 2^32`, which wraps
to `0` as `u32`), the encoder returns `Ok(Word(0xF9800000))` — byte-for-byte
identical to `[x0, #0]` — instead of rejecting the out-of-range offset. This is a
silent mis-assembly: the source text describes a huge offset but the emitted
instruction encodes offset 0.

For normal in-range offsets (the common case), the immediate form is correct, as
verified by the passing property `prop_prfm_immediate_matches_reference`.

## Evidence

```
$ cargo test --lib prop_encode_prfm_tests
test ... prop_prfm_large_offset_not_silently_truncated ... FAILED

minimal failing input: scaled = 4294967296 (imm = 34359738368)
  encoder returned Ok(Word(0xF9800000)) instead of Err
  ((imm/8) as u32 silently wrapped before the range check)
```

## Suggested fix

Range-check the scaled value as `i64` **before** the truncating cast:

```rust
let scaled = imm / 8;
if scaled > 0xFFF { return Err(format!("prfm: offset too large: {}", imm)); }
let imm12 = scaled as u32;
```

After this fix, `prop_prfm_large_offset_not_silently_truncated` passes; the other
four PRFM properties are unaffected.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/133
