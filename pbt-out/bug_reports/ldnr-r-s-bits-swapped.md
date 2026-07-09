# BUG REPORT — `encode_neon_ldnr`: LD2R / LD4R misencoded (R and S bits swapped)

- **Target:** `src/backend/arm/assembler/encoder/neon.rs :: encode_neon_ldnr`
- **Severity:** High — produces silently *wrong machine code* for two
  instructions; the bytes are still well-formed AArch64 but decode to a
  *different* instruction, so assembled LD2R/LD4R do the wrong thing at
  runtime with no assembler error.
- **Found by:** property-based tests in
  `src/backend/arm/assembler/encoder/neon_ldnr_pbt.rs` (differential oracle
  vs an independent ARM-ARM reference reconstruction, plus golden anchors
  captured from `llvm-mc-18 -triple=aarch64 -show-encoding`).

## Summary

`encode_neon_ldnr` selects the structure count (LD1R/LD2R/LD3R/LD4R) using
the wrong field. Per the ARM ARM, the "load single structure, replicate"
group is:

```
 31 30 29 28:24 23  22  21  20:16  15:13 12  11:10  9:5  4:0
  0  Q  0  01101 post L   R   Rm    opcode S  size   Rn   Rt
```

The count is encoded in **bit 21 (the R field)**, and **S (bit 12) is always
0** for this group:

| insn  | R (bit21) | opcode | S (bit12) |
|-------|-----------|--------|-----------|
| LD1R  | 0         | 110    | 0         |
| LD2R  | **1**     | 110    | 0         |
| LD3R  | 0         | 111    | 0         |
| LD4R  | **1**     | 111    | 0         |

The implementation instead stores the count in **S (bit 12)** and never sets
bit 21:

```rust
// opcode: ld1r=110, ld2r=110(S=1), ld3r=111, ld4r=111(S=1)   <-- WRONG
let (opcode, s_bit) = match num_structs {
    1 => (0b110u32, 0u32),
    2 => (0b110, 1),   // should be: R=1, S=0
    3 => (0b111, 0),
    4 => (0b111, 1),   // should be: R=1, S=0
    _ => return Err(...),
};
...
// bit 21 (R) is never OR-ed in; (s_bit << 12) is used instead.
let word = (q << 30) | (0b001101 << 24) | (if has_post {1} else {0} << 23)
    | (1 << 22) | (if has_post {rm} else {0} << 16)
    | (opcode << 13) | (s_bit << 12) | (size << 10) | (base << 5) | rt;
```

Effectively the R and S bits are **swapped**: for the *even* counts (LD2R,
LD4R) the encoder emits `bit12=1` (should be `0`) and `bit21=0` (should be
`1`). The odd counts (LD1R, LD3R) happen to come out right only because
their correct R and S are both 0.

## Evidence (llvm-mc-18 golden vs actual output)

| Instruction | Correct (llvm-mc-18) | `encode_neon_ldnr` | OK? |
|---|---|---|---|
| `ld2r {v0.16b,v1.16b}, [x1]`        | `0x4D60C020` | `0x4D40D020` | ✗ |
| `ld3r {v2.4s,v3.4s,v4.4s}, [x5]`    | `0x4D40E8A2` | `0x4D40E8A2` | ✓ |
| `ld4r {v6.8h..v9.8h}, [x10]`        | `0x4D60E546` | `0x4D40F546` | ✗ |
| `ld4r {v18.8b..v21.8b}, [x22], #4`  | `0x0DFFE2D2` | `0x0DDFF2D2` | ✗ |

In every failing case the XOR of actual-vs-correct is exactly `0x20001000`
(bits 21 and 12).

Decoding `0x4D40D020` (what the encoder emits for LD2R) per the ARM ARM
gives a different operand pattern than the programmer intended, so the
assembled program loads the wrong structure silently.

## Reproduce

```
cargo test --lib neon_ldnr_pbt
```

Failing tests (4):
- `prop_matches_arm_reference_encoding` — minimal shrunk input:
  `rt=0, rn=1, arr=8b, num_structs=2, post=false`
  (`LD2R {..}.8b [x1]: got 0x0D40D020 want 0x0D60C020`)
- `golden_ld2r_matches_llvm_mc`
- `golden_ld4r_matches_llvm_mc`
- `golden_ld4r_post_index_matches_llvm_mc`

Passing tests (6) confirm the rest of the encoder is sound:
`golden_ld3r_matches_llvm_mc`, `prop_fixed_top_bits_invariant`,
`prop_register_fields_place_correctly`, `prop_is_deterministic`,
`prop_negative_contract`, `prop_unsupported_arrangement_rejected`.

## Suggested fix

Encode the count in the R field (bit 21) and force S (bit 12) to 0:

```rust
let (opcode, r) = match num_structs {
    1 => (0b110u32, 0u32),
    2 => (0b110, 1),
    3 => (0b111, 0),
    4 => (0b111, 1),
    _ => return Err(format!("unsupported: ld{}r", num_structs)),
};
...
let word = (q << 30) | (0b001101 << 24) | (if has_post { 1 } else { 0 } << 23)
    | (1 << 22) | (r << 21) | (if has_post { rm } else { 0 } << 16)
    | (opcode << 13) | (size << 10) | (base << 5) | rt;
```

## Secondary note (not asserted as a failing test)

`encode_neon_ldnr` only inspects `operands[0]` and `operands[1]`. A
register post-index form `[Xn], Xm` (which the parser leaves as a trailing
`Operand::Reg` in `operands[2]`, since the parser only merges the immediate
variant into `MemPostIndex`) would be **silently ignored** and encoded as a
plain no-offset load — the same gap exists in `encode_neon_ld1r`, but is
handled correctly in `encode_neon_ld_st_multi`. Worth a follow-up to either
encode or explicitly reject it.
