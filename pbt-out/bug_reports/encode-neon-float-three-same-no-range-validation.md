# Bug Report: `encode_neon_float_three_same` silently accepts out-of-range parameters

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_float_three_same`
**Severity:** Medium

## Summary

`encode_neon_float_three_same` OR-shifts `u_bit`, `size_hi`, and `opcode` into the
instruction word without range validation. The three parameters are fixed-width
AArch64 encoding fields (`u_bit` = bit 29, 1-bit; `size_hi` feeds bit 23 of the
2-bit `size` field; `opcode` = bits 15–11, 5-bit), and the ARM ARM gives no
wrapping semantics for any of them. Out-of-range values are silently folded into
the word, corrupting adjacent architecturally-constant or adjacent fields instead
of being rejected.

## Root Cause

```rust
let size = (size_hi << 1) | sz;
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
    | (rm << 16) | (opcode << 11) | (1 << 10) | (rn << 5) | rd;
```

- `u_bit` is 1-bit (bit 29): value 2+ corrupts bit 30 (`Q`, the vector-arrangement marker)
- `size_hi` is 1-bit (feeds bit 23): value 2+ corrupts bit 24 — destroying the `01110` constant group at bits 28–24 → unallocated encoding
- `opcode` is 5-bit (bits 15–11): value 32+ corrupts bit 16 — the low bit of the `Rm` register field

## Reproduction

**Input:** `encode_neon_float_three_same(&[v0.4s, v1.4s, v2.4s], 2, 0, 0b11010)` (out-of-range `u_bit`)

**Expected:** `Err` — `u_bit` out of range (must be 0 or 1)

**Actual:** `Ok(Word(...))` — bit 30 (`Q`) corrupted, silently turning the `.4s`-shaped encoding into a `.2s`-shaped word

Analogous corruption occurs for `size_hi = 2` (bit 24) and `opcode = 32` (bit 16 / `Rm`).

## Impact

UNALLOCATED/corrupted encodings emitted without a diagnostic. All current call
sites (`fadd`, `fsub`, `fmul`, `fdiv`, `fmax`, `fmin`, `fmla`, `fmls`, `frecps`,
`frsqrts`, `fcmeq`, `fcmge`, `fcmgt`, `facge`, `facgt` at
`src/backend/arm/assembler/encoder/mod.rs:424–542`) pass correct-width constants,
so no shipped instruction is mis-encoded today. The defect is a latent foot-gun:
the function's signature accepts arbitrary `u32`, and any future caller passing a
wrong-width value emits a silently wrong word rather than failing. This is the
same unguarded OR-shift pattern documented for the sibling encoder
`encode_neon_scalar_three_same` (`neon-scalar-three-same-no-range-validation.md`),
and is shared by much of the `neon.rs` family (`encode_neon_three_same`,
`encode_neon_float_two_misc`, `encode_neon_float_cmp_zero`, `encode_neon_across`, …).

## Suggested Fix

Validate all parameters before encoding:

```rust
if u_bit > 1 { return Err("u_bit out of range (0-1)".into()); }
if size_hi > 1 { return Err("size_hi out of range (0-1)".into()); }
if opcode > 31 { return Err("opcode out of range (0-31)".into()); }
```

## Regression Property

Failing property: `rejects_out_of_range_params` (module
`backend::arm::assembler::encoder::neon_float_three_same_pbt`,
`#[ignore]`d pending fix — run with `--ignored`).

```rust
prop_assert!(encode_neon_float_three_same(&ops, 2, 0, 0b11010).is_err());  // u_bit
prop_assert!(encode_neon_float_three_same(&ops, 0, 2, 0b11010).is_err());  // size_hi
prop_assert!(encode_neon_float_three_same(&ops, 0, 0, 32).is_err());       // opcode
```

Confirmed to fail: `cargo test --lib neon_float_three_same_pbt::rejects_out_of_range_params -- --ignored`
→ `1 failed`. The five non-ignored properties (golden table, differential oracle,
field round-trip, fixed-bits invariant, arrangement/arity negative contract) all
pass.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/51
