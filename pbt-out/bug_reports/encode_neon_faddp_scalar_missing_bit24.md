# Bug Report: `encode_neon_faddp` scalar form emits wrong high constant (missing bit 24)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_faddp`
**Severity:** High

## Summary

`encode_neon_faddp`'s scalar form (`FADDP Sd, Vn.2S` / `FADDP Dd, Vn.2D`) emits an
**incorrect high constant byte**: every scalar `FADDP` word is `0x01000000` too
small. The implementation places the `11110` field at bits **28-24** plus an
explicit `0` at bit 23, yielding a top byte of `0x7E` (bit 24 = 0). The ARMv8-A
"Advanced SIMD scalar pairwise" template requires a `1` at bit 28 **and** `11110`
spanning bits **27-23**, i.e. **bit 24 must be 1**, giving a top byte of `0x7F`.
The result is a silently-emitted UNDEFINED/unallocated encoding; a real AArch64
CPU would raise `ILL` (SIGILL) on it.

The **vector form** (`FADDP Vd.T, Vn.T, Vm.T`) is **correct** and is verified
independently by a hand-derived golden table plus a differential reference
encoder.

## Root Cause

The "Advanced SIMD scalar pairwise" encoding template (ARMv8-A ARM) is:

```
0 1 U 1 11110 sz 1 1 0 0 0 opcode 1 0 Rn Rd
```

Field layout (bits 31 → 0):

| 31 | 30 | 29 | 28 | 27-23 | 22 | 21-17 | 16-12 | 11-10 | 9-5 | 4-0 |
|----|----|----|----|-------|----|-------|-------|-------|-----|-----|
| 0  | 1  | U  | 1  | 11110 | sz | 11000 | opcode|  10   | Rn  | Rd  |

For `FADDP` (U = 1) the top byte is therefore `0 1 1 1 1 1 1 1` = **`0x7F`**, with
`sz` at bit 22 and bit 23 = 0 (the last digit of `11110`).

Problematic code (`neon.rs`, `encode_neon_faddp`, scalar branch):

```rust
// 01 1 11110 0 sz 11000 01101 10 Rn Rd          <-- comment also wrong
let word = (0b01 << 30) | (1 << 29) | (0b11110 << 24) | (sz << 22)
    | (0b11000 << 17) | (0b01101 << 12) | (0b10 << 10) | (rn << 5) | rd;
```

`0b11110 << 24` shifts `11110` into bits **28-24** and clears bit 24; the
redundant extra `0` at bit 23 then produces `0 1 1 11110 0 sz` = top byte
**`0x7E`**. This is the classic one-bit-shift mistake: the vector form's
`0b01110 << 24` was copied and only the leading bit flipped, dropping the required
bit 24. Bit-by-bit (bits 28-22, `U=1`):

| bits 28-22 | correct (spec) | actual (code) |
|------------|----------------|---------------|
|            | `1 11110 sz`   | `11110 0 sz`  |
| bit 24     | **1**          | **0** ← bug   |
| bit 23     | 0              | 0             |
| bit 22     | sz             | sz            |

All other bits (Rn, Rd, `sz`, and the middle opcode field `11000 01101 10`,
bits 21-10) match the template; **only bit 24 is wrong.**

## Reproduction

Observed (current code) vs. correct:

| Instruction            | Emitted (wrong) | Correct       |
|------------------------|-----------------|---------------|
| `faddp s0, v1.2s`      | `0x7E30D820`    | `0x7F30D820`  |
| `faddp d0, v1.2d`      | `0x7E70D820`    | `0x7F70D820`  |
| `faddp s5, v6.2s`      | `0x7E30D8C5`    | `0x7F30D8C5`  |
| `faddp d31, v30.2d`    | `0x7E70DBDF`    | `0x7F70DBDF`  |

Failing characterization test (ignored, checked in at
`src/backend/arm/assembler/encoder/neon_faddp_pbt.rs::scalar_missing_bit24`):

```
$ cargo test --lib scalar_missing_bit24 -- --ignored
...
assertion `left == right` failed: scalar FADDP s0, v1.2s:
  top byte is 0x7E, spec requires 0x7F (bit 24 set); full word 0x7E30D820
  left: 126
 right: 127
```

## Impact

Every emitted scalar `FADDP` is an unallocated/UNDEFINED encoding. On real
AArch64 hardware it traps as an illegal instruction (SIGILL). The defect is
silent — the encoder returns `Ok(Word(...))` rather than `Err` — so it
mis-compiles any scalar pairwise float-add without diagnostic.

## Suggested Fix

Replace the buggy constant with the correctly-shifted scalar-pairwise prefix:

```rust
// 0 1 U 1 11110 sz 1 1 0 0 0 opcode 1 0 Rn Rd   (U=1, FADDP opcode)
let word = (0b01 << 30) | (1 << 29)        // 0 1 1 ...
         | (0b11111 << 23)                  // bit 28 = 1, bits 27-23 = 11110
         | (sz << 22)
         | (0b11000 << 17) | (0b01101 << 12) | (0b10 << 10)
         | (rn << 5) | rd;
```

i.e. change `0b11110 << 24` (`0x1E000000`) to `0b11111 << 23` (`0x0F800000`) and
drop the spurious bit-23 `0`. Equivalently, `| (1 << 24)` the existing constant.

## Regression Property

Failing property: `scalar_missing_bit24`

```rust
#[test]
#[ignore]
fn scalar_missing_bit24() {
    for &(rd, rn, arr) in &[(0u32, 1u32, "2s"), (0, 1, "2d"), (5, 6, "2s"), (31, 30, "2d")] {
        let dest = if arr == "2d" { format!("d{rd}") } else { format!("s{rd}") };
        let ops = vec![reg(&dest), va(rn, arr)];
        let w = match encode_neon_faddp(&ops) {
            Ok(EncodeResult::Word(w)) => w,
            other => panic!("expected Word, got {other:?}"),
        };
        assert_eq!(
            (w >> 24) & 0xFF, 0x7F,
            "scalar FADDP {dest}, v{rn}.{arr}: top byte is 0x{:02X}, \
             spec requires 0x7F (bit 24 set); full word 0x{w:08X}",
            (w >> 24) & 0xFF,
        );
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/193
