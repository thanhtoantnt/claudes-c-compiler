# Bug: `encode_neon_umov` sets Q=1 for 64-bit GP destination (UNDEFINED encoding)

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_umov`.

## Severity
Critical (silent mis-assembly → every `UMOV Xd, Vn.D[index]` emits an
UNDEFINED instruction word that will `#UD`/fault at runtime, with no
assembler diagnostic).

## Summary
The Q bit (bit 30) of the UMOV encoding is derived from the GP destination
register width:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let q = if is_64 { 1u32 } else { 0 };
```

This is wrong. UMOV writes a **general-purpose** register, not a vector, so the
SIMD-width Q bit is **architecturally forced to 0 in both forms**
(ARMv8 ARM, "Advanced SIMD copy" → `UMOV`):

```
0 Q 0 01110 000 imm5 0 0111 1 Rn Rd      (Q == 0 required)
```

For the 32-bit form (`UMOV Wd, Vn.<Ts>[index]`, `is_64 == false`) the code
happens to emit Q=0, which is correct. For the 64-bit form
(`UMOV Xd, Vn.D[index]`, `is_64 == true`) it emits **Q=1**, producing a word
with bit 30 set. No AArch64 instruction is allocated at
`0 1 0 01110 000 imm5 0 001111 Rn Rd`, so the output is UNDEFINED. The 64/128-bit
distinction that `Q` normally encodes does not apply because the destination is
a GP register; the element width is conveyed entirely by `imm5` (size sentinel).
The `is_64` flag is not needed for the encoding at all.

## Correct vs. actual encodings
| Mnemonic          | Correct (Q=0) | Actual (Q=1, buggy) |
|-------------------|---------------|---------------------|
| `umov w0, v0.s[0]` | `0x0E043C00` | `0x0E043C00` (ok)   |
| `umov w5, v2.b[7]` | `0x0E0F3C45` | `0x0E0F3C45` (ok)   |
| `umov x0, v0.d[0]` | `0x0E083C00` | `0x4E083C00` ✗      |
| `umov x9, v3.d[1]` | `0x0E183C69` | `0x4E183C69` ✗      |

The correct words match `llvm-mc`/`objdump` output for these mnemonics
(e.g. `umov x0, v0.d[0]` ⇒ `0x0e083c00`, bit 30 = 0).

## Reproduction
```text
umov x0, v0.d[0]   ->  Ok(Word(0x4E083C00))   // WRONG; should be 0x0E083C00 (Q=0)
umov x9, v3.d[1]   ->  Ok(Word(0x4E183C69))   // WRONG; should be 0x0E183C69 (Q=0)
umov w0, v0.s[0]   ->  Ok(Word(0x0E043C00))   // correct (is_64 == false -> Q==0)
```

Minimal counterexample found by property test: `dest_x = true, rd = 0,
rn = 0, elem = "b", idx_mod = 0`.

## Failing tests
`src/backend/arm/assembler/encoder/neon_umov_pbt.rs`
- `prop_q_bit_must_be_zero` (asserts `(word >> 30) & 1 == 0` for all valid inputs)
- `golden_umov` (asserts `0x0E083C00` for `umov x0, v0.d[0]`; code returns `0x4E083C00`)

## Suggested fix
Force Q=0 regardless of the destination register width (the width is already
encoded via the `imm5` size sentinel):

```rust
let (rd, _is_64) = get_reg(operands, 0)?;
// Q MUST be 0 for UMOV (GP destination); do not derive it from is_64.
// (Optional hardening: also reject Wd/D and Xd/{B,H,S} size/width mismatches.)
...
let word = (0b001110000u32 << 21) | (imm5 << 16) | (0b001111 << 10) | (rn << 5) | rd;
```
