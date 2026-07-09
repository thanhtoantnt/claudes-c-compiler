# LD1R post-index immediate is not validated against element size

## Status
Confirmed — reproduced by `neon_ld1r_pbt::ld1r_validates_post_index_offset`
(run with `cargo test --lib neon_ld1r_pbt::ld1r_validates_post_index_offset -- --ignored`).

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_ld1r`,
`Operand::MemPostIndex` arm:

```rust
Operand::MemPostIndex { base, offset } => {
    let rn = parse_reg_num(base).ok_or("invalid base reg")?;
    // LD1R post-index (immediate): 0 Q 0 01101 1 1 0 11111 110 0 size Rn Rt
    // Rm=11111 means post-index by element size
    let _ = offset; // offset must match element size, not encoded separately
    let word = (q << 30) | (0b001101 << 24) | (1 << 23) | (1 << 22)
        | (0b11111 << 16) | (0b110 << 13) | (size << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}
```

## What is wrong
The post-index immediate is bound but explicitly discarded (`let _ = offset;`).
For `LD1R`, the A‑profile ARM ARM specifies that the optional post-index
immediate **must equal the element size in bytes** of the arrangement:

| arrangement      | size | required immediate |
|------------------|------|--------------------|
| `.8b` / `.16b`   | 00   | `#1`               |
| `.4h` / `.8h`    | 01   | `#2`               |
| `.2s` / `.4s`    | 10   | `#4`               |
| `.1d` / `.2d`    | 11   | `#8`               |

Any other value is UNPREDICTABLE and a correct assembler rejects it. This
encoder hard-codes `Rm = 11111` (the "immediate, by element size" encoding)
unconditionally, so it emits a well-formed word for a **wrong** immediate
instead of returning `Err`. The author's own comment ("offset must match
element size") states the precondition but the code never enforces it.

## Impact
Silent mis-assembly. A typo such as `ld1r {v0.16b}, [x1], #2` (intending the
1‑byte element size of `.16b`) produces a valid-looking instruction that, at
run time, post-increments `x1` by **1** (the encoded element size), not by the
**2** the programmer wrote. The assembler's output disagrees with its input
with no diagnostic. Compared to a reference assembler (`as`/`llvm-mc`), which
emits `Error: immediate must be 1`, this is a correctness gap.

## Reproduction
```
ld1r {v0.8b}, [x0], #2     // offset 2, but .8b requires #1
```
Minimal failing input found by proptest: `rt = 0, rn = 0, arr = "8b", bad_offset = 2`.
`encode_neon_ld1r` returns `Ok(Word(0x0DDFC400))` (the same word it would emit
for the correct `#1`) instead of `Err`.

## Suggested fix
After parsing `offset` in the `MemPostIndex` arm, validate it against the
element size derived from the arrangement, e.g.:

```rust
Operand::MemPostIndex { base, offset } => {
    let rn = parse_reg_num(base).ok_or("invalid base reg")?;
    let elem_bytes: i64 = 1 << size; // size is 0..3 -> 1,2,4,8
    if offset != elem_bytes {
        return Err(format!(
            "ld1r: post-index immediate #{} must equal element size #{} for .{}",
            offset, elem_bytes, arr
        ));
    }
    let word = (q << 30) | (0b001101 << 24) | (1 << 23) | (1 << 22)
        | (0b11111 << 16) | (0b110 << 13) | (size << 10) | (rn << 5) | rt;
    Ok(EncodeResult::Word(word))
}
```

## Notes on coverage
The core LD1R encoding is correct — the differential property
`ld1r_matches_reference_encoder` (all 8 arrangements × v0–v31 × Rn 0–31 × both
addressing modes) passes, and the no-offset `.4s` case matches the canonical
`0x4D40C820`. Only the post-index immediate validation is missing.
