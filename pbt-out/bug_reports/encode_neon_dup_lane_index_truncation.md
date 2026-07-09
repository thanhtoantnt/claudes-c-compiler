# Bug: `encode_neon_dup` silently truncates out-of-range vector lane indices

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_dup`
(element/broadcast form: `DUP Vd.T, Vn.Ts[index]`).

## Severity
High (silent mis-assembly → wrong machine code with no diagnostic).

## Summary
When encoding `DUP Vd.T, Vn.<size>[index]`, the lane index is packed into the
`imm5` field using bitwise AND masks instead of range validation:

```rust
let imm5 = match elem_size.as_str() {
    "b" => ((*index & 0xF) << 1) | 0b00001,  // 4 index bits -> max 15
    "h" => ((*index & 0x7) << 2) | 0b00010,  // 3 index bits -> max 7
    "s" => ((*index & 0x3) << 3) | 0b00100,  // 2 index bits -> max 3
    "d" => ((*index & 0x1) << 4) | 0b01000,  // 1 index bit  -> max 1
    _ => return Err(...),
};
```

An index that exceeds the field width is **silently masked** rather than
rejected. Per the ARM ARM, the `imm5` field for DUP(element) is a fixed-width
encoding of size+index; values that do not fit are reserved/UNDEFINED, not
wrap-on-overflow. The masks therefore cause the assembler to emit a *different,
valid-looking instruction* with no error.

## Reproduction
```text
DUP V0.16b, V1.b[16]   ->  Ok(Word(0x4E010400))   // same as V1.b[0]  (16 & 0xF == 0)
DUP V0.16b, V1.b[17]   ->  Ok(Word(0x4E010420))   // same as V1.b[1]  (17 & 0xF == 1)
DUP V0.8h,  V1.h[8]    ->  Ok(Word(...))          // aliases V1.h[0]
DUP V0.4s,  V1.s[4]    ->  Ok(Word(...))          // aliases V1.s[0]
DUP V0.2d,  V1.d[2]    ->  Ok(Word(...))          // aliases V1.d[0]
DUP V0.2d,  V1.d[3]    ->  Ok(Word(...))          // aliases V1.d[1]
```

Minimal counterexample found by property test: `index = 16` for element size
`.b` returns `Ok(Word(0x4E010400))` instead of `Err(...)`.

## Property test (failing)
`src/backend/arm/assembler/encoder/neon.rs`, module `dup_pbt_extra_tests`,
property `prop_out_of_range_lane_index_must_error`:

```rust
// indices exceeding the per-size imm5 field width (b>15, h>7, s>3, d>1)
// are unrepresentable for ANY arrangement and must be rejected.
let cases: &[(u32, &str, &str)] = &[
    (15 + oob, "16b", "b"),
    (7 + oob,  "8h",  "h"),
    (3 + oob,  "4s",  "s"),
    (1 + oob,  "2d",  "d"),
];
for &(index, dest, es) in cases {
    let res = encode_neon_dup(&elem_ops(0, dest, 1, es, index));
    prop_assert!(res.is_err(), ...);  // currently Ok -> test fails
}
```

Result: 4 of the 5 new properties pass (golden full-word oracles for both forms,
full-word differential reference, and general-vs-element non-aliasing). Only this
negative-contract property fails, confirming the bug is isolated to lane-index
range handling.

## Suggested fix
Validate `index` against the per-size maximum before masking, mirroring the
existing element-index validation in `encode_neon_elem_long`:

```rust
let max_index = match elem_size {
    "b" => 15u32,
    "h" => 7,
    "s" => 3,
    "d" => 1,
    _ => return Err(format!("unsupported dup element size: {}", elem_size)),
};
if *index > max_index {
    return Err(format!("dup: element index {} out of range for .{}", index, elem_size));
}
```

(Strictly, the architectural maximum also depends on the destination arrangement
/Q bit — e.g. `.b` in an `8b` (Q=0) destination only permits indices 0–7 — so a
fully correct fix should additionally bound the index against `arr_d`.)

## Note
The GP-register form (`DUP Vd.T, Rn`) is unaffected; it has no lane index. The
sibling function `encode_neon_ins` uses the same `(*index & mask)` idiom and is
likely affected by the same class of bug — worth a follow-up property suite.
