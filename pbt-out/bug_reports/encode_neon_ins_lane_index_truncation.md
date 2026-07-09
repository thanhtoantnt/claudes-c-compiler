# Bug Report: `encode_neon_ins` silently truncates out-of-range NEON lane indices

**Severity:** Medium (assembler correctness — emits a wrong, architecturally UNPREDICTABLE encoding with no diagnostic)

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_ins` (lines ~560–605)

## Summary

`encode_neon_ins` computes the `imm5` lane-index field by **bit-masking** the
supplied lane index without ever validating its range:

```rust
// general form (INS Vd.Ts[i], Xn)        neon.rs:560-563
"b" => ((*index & 0xF) << 1) | 0b00001,
"h" => ((*index & 0x7) << 2) | 0b00010,
"s" => ((*index & 0x3) << 3) | 0b00100,
"d" => ((*index & 0x1) << 4) | 0b01000,
```

The same masking pattern applies to the element-to-element form
(`INS Vd.Ts[dst], Vn.Ts[src]`) for both `imm5` (destination index) and `imm4`
(source index), neon.rs:578–595.

Because the high bits of the index are silently discarded, an **out-of-range**
lane index wraps instead of being rejected. Per the ARMv8 ARM the lane index is
constrained per element size (`.b` → [0,15], `.h` → [0,7], `.s` → [0,3],
`.d` → [0,1]); an encoding with an out-of-range `imm5`/`imm4` is
UNPREDICTABLE. No AArch64 spec defines such wrapping as intentional.

## Spec reference

`llvm-mc-18 -triple=aarch64` rejects every out-of-range lane, e.g.:

```
$ echo "ins v0.b[16], w1" | llvm-mc-18 -triple=aarch64 -show-encoding -assemble
<stdin>:1:9: error: vector lane must be an integer in range [0, 15].
$ echo "ins v0.h[8], v1.h[0]" | llvm-mc-18 ...
error: vector lane must be an integer in range [0, 7].
$ echo "ins v0.s[4], w1"      | llvm-mc-18 ...
error: vector lane must be an integer in range [0, 3].
$ echo "ins v0.d[2], v1.d[0]" | llvm-mc-18 ...
error: vector lane must be an integer in range [0, 1].
```

## Reproduction (property test)

`src/backend/arm/assembler/encoder/neon_ins_pbt.rs`,
`prop_rejects_out_of_range_lane_index` — expects `Err` for an out-of-range lane.

**Minimal failing input:** `elem = "b", over = 1`  →  lane index `16`.

```
Test failed: out-of-range lane [16] for .b (max 15) must be rejected,
not silently truncated
minimal failing input: elem = "b", over = 1
```

Actual behaviour: `encode_neon_ins(&[ v0.b[16], x1 ])` returns
`Ok(Word(0x4e011de0))` — identical to `v0.b[0]` because `16 & 0xF == 0`. The
caller has no way to know the encoding is invalid.

## Impact

* An out-of-range lane (e.g. from a codegen bug, macro, or hand-written asm)
  assembles without error into an UNPREDICTABLE instruction, producing silent
  miscompilation rather than a loud assembler error.
* This is the exact "silent masking/truncation of lanes" anti-pattern: the
  bitmask looks like deliberate width-limiting but actually hides invalid input.

## Suggested fix

Validate the index against the per-size maximum *before* packing, returning
`Err` on overflow (mirrors what `llvm-mc` does):

```rust
let (shift, sentinel, max) = match elem_size.as_str() {
    "b" => (1u32, 0b00001, 15u32),
    "h" => (2,    0b00010, 7),
    "s" => (3,    0b00100, 3),
    "d" => (4,    0b01000, 1),
    _ => return Err(format!("unsupported ins element size: {}", elem_size)),
};
if *index > max {
    return Err(format!("ins: lane index {} out of range for .{} (max {})",
                       index, elem_size, max));
}
let imm5 = (index << shift) | sentinel;
```

Apply the same range check to both forms (general + element-to-element) and to
the source-lane index of the element form.

## Evidence the rest of the encoder is correct

The same property suite confirms, against `llvm-mc-18` golden encodings, that
for **all in-range inputs** both INS forms are byte-exact correct:

| Instruction (llvm-mc golden) | Encoder word |
| --- | --- |
| `ins v0.s[0], w1`  = 0x4e041c20 | ✅ 0x4e041c20 |
| `ins v31.d[1], x30` = 0x4e181fdf | ✅ 0x4e181fdf |
| `ins v5.s[2], v7.s[3]` = 0x6e1464e5 | ✅ 0x6e1464e5 |
| `ins v4.d[1], v6.d[0]` = 0x6e1804c4 | ✅ 0x6e1804c4 |

Only the missing range check is defective; the bit-layout logic itself is right.
