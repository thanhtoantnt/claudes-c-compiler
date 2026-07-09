# Bug Report — `encode_bfm` accepts out-of-range `immr`/`imms` and silently corrupts the opcode

**File:** `src/backend/arm/assembler/encoder/bitfield.rs`
**Function:** `encode_bfm` (Bitfield Move — `BFM`)
**Severity:** High (silent miscompilation: emits an instruction with a corrupted `N` / opcode field instead of failing at assembly time)
**Status:** Confirmed by property-based test (expected-fail).

## Summary

`encode_bfm` casts the parsed immediates with `as u32` and ORs them directly into
the 32-bit instruction word without any range validation:

```rust
pub(crate) fn encode_bfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let immr = get_imm(operands, 2)? as u32;   // ← no range check
    let imms = get_imm(operands, 3)? as u32;   // ← no range check
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    // BFM: sf 01 100110 N immr imms Rn Rd
    let word = (sf << 31) | (0b01 << 29) | (0b100110 << 23) | (n << 22)
             | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Why it is a bug

Per the ARM Architecture Reference Manual (Bitfield encoding, `BFM`/`UBFM`/`SBFM`):

```
sf  opc[30:29]  100110  N[22]  immr[21:16]  imms[15:10]  Rn[9:5]  Rd[4:0]
```

- `immr` and `imms` are **6-bit fields** — architecturally `0..=63`.
- The encoding is **CONSTRAINED**: `N == sf`.
- An out-of-range immediate does not simply "truncate the field"; the
  overflow bits spill into **adjacent fixed/structural fields**:
  - `immr = 64` → `64 << 16 = 0x0040_0000` sets **bit 22 = `N`**, violating
    `N == sf` and producing a structurally UNDEFINED / UNALLOCATED encoding.
  - `imms = 64` → `64 << 10 = 0x0001_0000` sets bit 16 (the low bit of the
    `immr` field), corrupting the field value.
  - A negative immediate (e.g. `-1`) wraps via `as u32` to `0xFFFF_FFFF`,
    which ORs `1`s across `sf`, `opc`, the fixed `100110`, `N`, `immr`,
    `imms`, `Rn`, and `Rd` simultaneously — total opcode corruption.

A correct assembler must reject these inputs with `Err`, not emit a silently
mangled word. The same defect exists in the sibling functions
`encode_ubfm` and `encode_sbfm` (and the alias encoders derived from them).

## Reproduction

Property test added in `src/backend/arm/assembler/encoder/bitfield.rs`,
module `prop_encode_bfm_tests::prop_rejects_out_of_range_immediates`
(expected to fail — documents the contract violation):

```
cargo test --lib prop_encode_bfm
```

Minimal failing input (proptest-shrunk):

```
immr = 64   →  Ok(Word(3007316000))   ; expected Err
             word = 0xB340_0000 = sf=1 opc=01 100110 N=1 immr=0 ...   (N silently flipped)
```

For a 64-bit register (`sf=1`, so `N` *should* be 1) the corruption is
invisible at the `N` bit, but `imms=64` and any `immr`/`imms` ≥ 64 still
produce a word whose decoded `immr`/`imms` no longer match the assembler
input — i.e. the encoded instruction does not do what the source says. For a
32-bit register (`sf=0`, `N` *should* be 0), `immr=64` flips `N` to 1,
yielding an **unallocated** encoding.

## Properties written for `encode_bfm`

| # | Property | Oracle | Result |
|---|----------|--------|--------|
| A | `prop_bfm_field_placement` | structural (field positions, opc=01, N==sf) | ✅ pass |
| B | `prop_bfm_xor_siblings` | differential vs UBFM/SBFM (only opc[30:29] differs) | ✅ pass |
| C | `prop_bfm_equals_bfxil_alias` | differential vs `BFXIL` alias | ✅ pass |
| D | `prop_width_changes_only_sf_and_n` | register-width differential | ✅ pass |
| E | `prop_rejects_out_of_range_immediates` | **negative / error contract** | ❌ **FAIL (this bug)** |
| F | `prop_rejects_malformed_operands` | negative / error contract (missing/wrong types) | ✅ pass |

## Suggested fix

Validate `immr` and `imms` before encoding (and enforce `N == sf`):

```rust
if immr > 63 {
    return Err(format!("BFM: immr {} out of range [0,63]", immr));
}
if imms > 63 {
    return Err(format!("BFM: imms {} out of range [0,63]", imms));
}
```

`get_imm` returns an `i64`; reject negatives there (or here) before the
`as u32` cast. Apply the same guard to `encode_ubfm`, `encode_sbfm`, and the
alias encoders (`encode_ubfx`, `encode_sbfx`, `encode_bfi`, `encode_bfxil`,
`encode_sbfiz`, `encode_ubfiz`).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/158
