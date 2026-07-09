# Bug: `encode_neon_umov` silently truncates out-of-range vector lane indices

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_umov`.

## Severity
High (silent mis-assembly → wrong machine code with no diagnostic).

## Summary
When packing the lane index into the `imm5` field, the index is masked instead
of range-checked:

```rust
let imm5 = match elem_size.as_str() {
    "b" => ((*index & 0xF) << 1) | 0b00001,  // 4 index bits -> max 15
    "h" => ((*index & 0x7) << 2) | 0b00010,  // 3 index bits -> max 7
    "s" => ((*index & 0x3) << 3) | 0b00100,  // 2 index bits -> max 3
    "d" => ((*index & 0x1) << 4) | 0b01000,  // 1 index bit  -> max 1
    _ => return Err(...),
};
```

The masks equal the maximum valid lane for each size, so an in-range index
passes through unchanged, but any out-of-range index is **silently aliased**
onto a smaller index rather than rejected. Per the ARMv8 ARM the valid lane
ranges are exactly `.b->[0,15] .h->[0,7] .s->[0,3] .d->[0,1]`; `llvm-mc-18`
rejects out-of-range lanes (e.g. `umov w0, v0.b[16]` → "vector lane must be an
integer in range [0, 15]"). No AArch64 spec defines wrapping/truncation as
intentional here, so this is a recurring pattern bug shared with
`encode_neon_dup` / `encode_neon_ins` (see sibling reports).

## Reproduction
```text
umov w0, v0.b[16]  ->  Ok(Word(0x0E013C00))   // aliases v0.b[0]  (16 & 0xF == 0)
umov w0, v0.b[17]  ->  Ok(Word(0x0E013C20))   // aliases v0.b[1]  (17 & 0xF == 1)
umov w0, v0.h[8]   ->  Ok(Word(...))          // aliases v0.h[0]
umov w0, v0.s[4]   ->  Ok(Word(...))          // aliases v0.s[0]
umov x0, v0.d[2]   ->  Ok(Word(...))          // aliases v0.d[0]  (ignoring the separate Q-bit bug)
```

Minimal counterexample found by property test: `elem = "b", over = 1`
→ lane index `16` for `.b` (max 15).

## Failing test
`src/backend/arm/assembler/encoder/neon_umov_pbt.rs` →
`prop_rejects_out_of_range_lane_index` (asserts `is_err()` for
`max_lane + over`).

## Suggested fix
Validate the index against the per-size maximum before encoding and return
`Err` otherwise, e.g.:

```rust
"b" if *index <= 15 => (*index << 1) | 0b00001,
"h" if *index <= 7  => (*index << 2) | 0b00010,
"s" if *index <= 3  => (*index << 3) | 0b00100,
"d" if *index <= 1  => (*index << 4) | 0b01000,
_ => return Err(format!("umov lane index out of range for .{}", elem_size)),
```
