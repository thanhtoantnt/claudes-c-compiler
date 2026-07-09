# Bug Report: `encode_neon_addv` assembles the ADDV opcode one bit too low (every word is wrong)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_addv`
**Severity:** High

## Summary

`encode_neon_addv` mis-assembles the fixed opcode field of the `ADDV`
(Advanced SIMD across-lanes) encoding. As a result **every** instruction word
it produces is wrong — the emitted 32-bit word does not decode as `ADDV` (it
lands on an unrelated / UNALLOCATED encoding). Any backend relying on this
function to emit a vector reduction will generate incorrect machine code.

## Root Cause

The ARMv8-A ARM layout for `ADDV` is:

```
0 Q 0 01110 size 11000 11011 10 Rn Rd
                -----21-17  16-12 11-10
```

The implementation builds the middle constant as:

```rust
// ADDV: 0 Q 0 01110 size 11000 11011 10 Rn Rd
let word = (q << 30) | (0b001110 << 24) | (size << 22) | (0b11000 << 17)
    | (0b110111 << 10) | (rn << 5) | rd;
//                ^^^^^^^^^^^^^^^^
```

`0b110111` is only **6 bits** wide, so `(0b110111 << 10)` occupies bits
**15–10** (`110111`). The documented field is the **7-bit** value `1101110`
spanning **bits 16–10** (`11000 [opcode=11011] [10]`). The constant is shifted
one bit too far to the right, so:

| bit | required | actual |
|-----|----------|--------|
| 16  | 1        | **0**  |
| 10  | 0        | **1**  |

The correctly-assembled form is the same one used by the sibling
`encode_neon_across`:

```rust
(0b11000 << 17) | (0b11011 << 12) | (0b10 << 10)
```

i.e. `encode_neon_addv(ops)` should equal
`encode_neon_across(ops, /*u_bit*/ 0, /*opcode*/ 0b11011)` for all valid
operands.

## Reproduction

```text
$ cargo test --lib neon_addv -- --ignored
test ...::addv_matches_golden_table ... FAILED
  addv v0.4s, v1.4s: got 0x4EB0DC20, want 0x4EB1B820
  opcode bits 16-12: left `13` (0b01101), right `27` (0b11011)
test ...::addv_matches_reference_encoder ... FAILED
test ...::addv_fixed_bits_bits_are_constant ... FAILED
```

Concrete word comparison (`addv v0.4s, v1.4s`, Q=1 size=10):

|            | bit 31 | 30(Q) | 29(U) | 28-24 | 23-22 | 21-17 | 16-12 | 11-10 | 9-5 | 4-0 | hex        |
|------------|--------|-------|-------|-------|-------|-------|-------|-------|-----|-----|------------|
| expected   | 0      | 1     | 0     | 01110 | 10    | 11000 | 11011 | 10    | 1   | 0   | `0x4EB1B820` |
| emitted    | 0      | 1     | 0     | 01110 | 10    | 11000 | 01101 | 11    | 1   | 0   | `0x4EB0DC20` |

All six golden cases in `neon_addv_pbt.rs` fail identically.

## Impact

High. Every `ADDV` emitted by the ARM backend is currently malformed; the
word does not decode as a valid across-lanes instruction (opcode field reads
`0b01101` instead of `0b11011`, and bit 10 is inverted). This is silent
mis-compilation of a vector reduction. No range/truncation workaround masks
it — the bug is in the fixed opcode field, so it triggers on every input.

## Suggested Fix

Replace the constant term in `encode_neon_addv`:

```rust
// before
| (0b110111 << 10)
// after  (opcode 11011 at bits 16-12, fixed "10" at bits 11-10)
| (0b11011 << 12) | (0b10 << 10)
```

After the fix, the three `#[ignore]`d properties in `neon_addv_pbt.rs`
(`addv_matches_golden_table`, `addv_matches_reference_encoder`,
`addv_fixed_bits_are_constant`) pass and their `#[ignore]` attributes can be
removed.

## Regression Property

Failing property: `addv_matches_golden_table` (also `addv_matches_reference_encoder`, `addv_fixed_bits_are_constant`)

```rust
#[test]
#[ignore]
fn addv_matches_golden_table() {
    for &(rd, rn, arr, expected) in GOLDEN {
        let ops = vec![va(rd, arr), va(rn, arr)];
        let got = word_of(encode_neon_addv(&ops));
        assert_eq!(
            got, expected,
            "addv v{rd}.{arr}, v{rn}.{arr}: got 0x{got:08X}, want 0x{expected:08X}",
        );
    }
}
```

## Notes on secondary behavior (out of scope of this fix)

`encode_neon_addv` accepts `.1d`/`.2d` (size=0b11) and `.2s`, arrangements
that are not architecturally valid for ADDV, without returning `Err`. This
matches the pre-existing pattern documented for `encode_neon_mla`
(`encode_neon_mla_unallocated_doubleword.md`) and is a separate
negative-contract gap.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/191
