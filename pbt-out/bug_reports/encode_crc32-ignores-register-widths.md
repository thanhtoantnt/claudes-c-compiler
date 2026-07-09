# BUG: `encode_crc32` ignores register widths — accepts illegal mnemonic/width combos

**File:** `src/backend/arm/assembler/encoder/bitfield.rs`
**Function:** `encode_crc32(mnemonic: &str, operands: &[Operand])`
**Severity:** Medium (silent mis-encoding; produces an UNPREDICTABLE/UNALLOCATED instruction word instead of an assembler error)
**Test that catches it:** `prop_encode_crc32_tests::prop_rejects_width_mismatch_per_variant` (EXPECTED-TO-FAIL negative contract)

## Summary

`encode_crc32` discards the width flag of **every** register operand and derives `sf` purely from the mnemonic suffix. Consequently it silently emits a `Word` for any combination of mnemonic and register width that the ARM ARM requires an assembler to **reject**. A real assembler (GAS / `llvm-mc`) errors on these inputs; this encoder accepts them and produces a mis-sized instruction.

## Root cause

```rust
pub(crate) fn encode_crc32(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // width discarded
    let (rn, _) = get_reg(operands, 1)?;   // width discarded
    let (rm, _) = get_reg(operands, 2)?;   // width discarded
    ...
    let (sf, sz) = match mnemonic {         // sf driven ONLY by mnemonic
        "crc32b" | "crc32cb" => (0u32, 0b00u32),
        "crc32h" | "crc32ch" => (0, 0b01),
        "crc32w" | "crc32cw" => (0, 0b10),
        "crc32x" | "crc32cx" => (1, 0b11),
        _ => (0, 0b00),
    };
    ...
}
```

`get_reg` already returns the parsed width (`is_64`, true for `x`, false for `w`) as its second tuple element, but all three call sites bind it to `_`.

## The register-width contract (ARM ARM, CRC32 / CRC32C)

The family admits exactly one legal register-width combination per size class:

| Variant | Rd | Rn | Rm |
|---|---|---|---|
| `crc32{b,h,w}`, `crc32c{b,h,w}` | `Wd` (32-bit) | `Wn` (32-bit) | `Wm` (32-bit) |
| `crc32{x}`, `crc32c{x}` | `Xd` (64-bit) | `Wn` (32-bit) | `Xm` (64-bit) |

Anything else is **UNPREDICTABLE / UNALLOCATED** and must be rejected.

## Reproduction

`cargo test --lib prop_rejects_width_mismatch_per_variant` fails at the first case:

```
"crc32b" with Rd=X0, Rn=W1, Rm=W2 (slot 0 mismatched) must be rejected, got Ok(Word(448938016))
```

Concretely, each of the following returns `Ok(Word(...))` but should return `Err`:

| Mnemonic | Operands | Why illegal |
|---|---|---|
| `crc32b` | `x0, w1, w2` | `crc32b` requires `Wd`; Rd is X |
| `crc32b` | `w0, x1, w2` | requires `Wn`; Rn is X |
| `crc32b` | `w0, w1, x2` | requires `Wm`; Rm is X |
| `crc32x` | `w0, w1, w2` | requires `Xd`; Rd is W |
| `crc32x` | `x0, x1, x2` | requires `Wn`; Rn is X |
| `crc32x` | `x0, w1, w2` | requires `Xm`; Rm is W |
| (same for `crc32cb/ch/cw/cx`) | … | … |

Note: a passing positive reference (`prop_legal_width_combination_matches_spec`) and a known-constant anchor (`prop_crc32_known_constants`: `CRC32W W0,W0,W0 = 0x1AC0_4800`, `CRC32X X0,W0,X0 = 0x9AC0_4C00`, diff `0x8000_0400`) confirm the encoder is correct for the *legal* widths — the bug is purely the missing validation of illegal ones.

## Suggested fix

After parsing, validate each width against the variant:

```rust
let is_x = matches!(mnemonic, "crc32x" | "crc32cx");
let (rd, rd_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
let (rm, rm_64) = get_reg(operands, 2)?;
if rd_64 != is_x { return Err(format!("{} requires {}d register", mnemonic, if is_x { 'X' } else { 'W' })); }
if rn_64         { return Err(format!("{} requires Wn (32-bit) register", mnemonic)); } // Rn is ALWAYS W
if rm_64 != is_x { return Err(format!("{} requires {}m register", mnemonic, if is_x { 'X' } else { 'W' })); }
```

(Then `sf` can be derived from `is_x` rather than from the `match`, which already agrees.)

## Related (pre-existing) finding

`encode_crc32` also silently accepts **unknown mnemonics** (the `_ => (0, 0b00)` match arm, combined with `mnemonic.contains("crc32c")`), e.g. `crc32`, `crc32d`, `crc32cd`, `nop`, `""`. That is documented by the pre-existing `prop_rejects_unknown_mnemonics` test and is the same class of "no validation" bug.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/142
