# Bug Report — `encode_rev32` (scalar) ignores register width, always emits the 64-bit encoding

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_rev32`
**Severity:** High (silent miscompilation of every 32-bit `REV32 Wd, Wn`)

## Summary

The scalar branch of `encode_rev32` discards the destination register width and
hardcodes the 64-bit opcode bits, so `REV32 Wd, Wn` is encoded as if it were
`REV32 Xd, Xn`:

```rust
pub(crate) fn encode_rev32(operands: &[Operand]) -> Result<EncodeResult, String> {
    // ... NEON vector branch omitted ...
    let (rd, _) = get_reg(operands, 0)?;          // <-- `is_64` discarded
    let (rn, _) = get_reg(operands, 1)?;
    // REV32 is 64-bit only: 1 1 0 11010110 00000 000010 Rn Rd
    let word = ((1u32 << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000010 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The comment "REV32 is 64-bit only" is factually wrong, and the code reflects that
wrong assumption: `1u32 << 31` (sf=1) and `0b000010` (opc) are hard constants.

## ARM ARM reference

`REV32` (scalar) uses the *Data-processing (1 source)* encoding:

```
sf 1 0 11010110 00000 opc[15:10] Rn Rd
```

The `(sf, opc)` decode for the REV/REV32 group is a bijection over
`opc ∈ {000010, 000011}`, with REV and REV32 **mirrored** in `sf`:

| Instruction    | sf | opc[15:10] | base word |
|----------------|----|------------|-----------|
| REV  32-bit    | 0  | `000010`   | `0x5AC00800` |
| REV  64-bit    | 1  | `000011`   | `0xDAC00C00` |
| REV32 32-bit   | 0  | `000011`   | `0x5AC00C00` |
| REV32 64-bit   | 1  | `000010`   | `0xDAC00800` |

(The sibling `encode_rev` in the same file gets this right:
`opc = if is_64 { 0b000011 } else { 0b000010 }`.)

`REV32` is therefore defined in **both** widths:

* `REV32 <Wd>, <Wn>` → `sf=0, opc=000011`
* `REV32 <Xd>, <Xn>` → `sf=1, opc=000010`

## Consequence

Because the encoder hardcodes `sf=1` and `opc=000010`:

* `REV32 Xd, Xn` is encoded **correctly** → `0xDAC00800 | (Rn<<5) | Rd`.
* `REV32 Wd, Wn` is encoded **incorrectly**: it emits the 64-bit word
  `0xDAC00800 | (Rn<<5) | Rd` instead of `0x5AC00C00 | (Rn<<5) | Rd`.
  The output is wrong in **both** `sf` (bit 31, should be 0) and `opc` (bits
  [15:10], should be `000011`).

A disassembler/assembler fed this word would read it back as `rev32 Xd, Xn`
(silently widening the operation from 32 to 64 bits) rather than the intended
`rev32 Wd, Wn` — a silent miscompilation with no error reported.

## Minimal failing case

```
REV32 w0, w0   ->  expected 0x5AC00C00, produced 0xDAC00800
```

(`0xDAC00800` is the encoding of `rev32 x0, x0`.)

## Suggested fix

Derive `sf` and `opc` from the register width, mirroring `encode_rev`:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;
let sf = sf_bit(is_64);
let opc: u32 = if is_64 { 0b000010 } else { 0b000011 }; // REV32 mirrors REV
let word = (sf << 31) | (1 << 30) | (0b011010110 << 21)
    | (opc << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

## Note on the NEON (vector) branch

The vector branch is **correct**: `0 Q 1 01110 size 1 00000 0000 10 Rn Rd` is
encoded as specified (verified by `prop_neon_field_placement`). One secondary
gap worth noting: `REV32 <Vd>.<T>, <Vn>.<T>` is only architecturally valid for
`size ∈ {00 (bytes), 01 (halfwords)}`; the code accepts any arrangement
`neon_arr_to_q_size` recognises (including `2s`/`4s`→size `10` and
`1d`/`2d`→size `11`), producing **UNALLOCATED** encodings without error. That
is a separate, lower-severity missing-validation issue not exercised by the
failing property below.

## Evidence

Property-based test module `prop_encode_rev32_tests` in
`src/backend/arm/assembler/encoder/bitfield.rs`:

| Property | Oracle | Result |
|----------|--------|--------|
| `prop_scalar_field_placement_64bit` | structural (X form) | **pass** |
| `prop_scalar_matches_arm_reference` | ARM ARM reference, both widths | **FAIL** (W form) |
| `prop_neon_field_placement` | structural (vector form) | pass |
| `prop_rejects_malformed_operands` | negative contract | pass |
| `prop_deterministic` | purity | pass |

Minimal failing input reported by proptest: `is_64 = false, rd = 0, rn = 0`
→ `REV32 w0, w0`: expected `0x5AC00C00`, got `0xDAC00800`.
