# Bug Report — `encode_sbfm` silently accepts out-of-range `immr`/`imms`

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_sbfm`
**Severity:** Medium (silent miscompilation — produces a valid-looking but semantically wrong instruction word, no error surfaced to the assembler caller)

## Summary

`encode_sbfm` reads its two immediate operands with `get_imm(...)? as u32` and
directly ORs them into the 32-bit encoding word with **no range validation**:

```rust
let immr = get_imm(operands, 2)? as u32;
let imms = get_imm(operands, 3)? as u32;
...
let word = (sf << 31) | (0b100110 << 23) | (n << 22)
         | (immr << 16) | (imms << 10) | (rn << 5) | rd;
```

Per the ARM Architecture Reference Manual (Bitfield / SBFM encoding), `immr` and
`imms` are **6-bit fields** occupying bits `[21:16]` and `[15:10]` respectively.
The architecturally valid range is therefore `0..=63` (and further constrained by
register width: for 32-bit, `0..=31`). An assembler **must reject** any value
outside this range.

The current code performs no check, so:

* `immr = 64` produces `(64u32 << 16) = 0x0040_0000`, which is **bit 22 — the `N`
  field**. The overflow silently corrupts the `N`/`sf`-constrained bit rather
  than being rejected.
* `imms = 64` produces `(64u32 << 10) = 0x0001_0000`, overflowing into the `Rn`
  field region / reserved space.
* Negative immediates (e.g. `-3`) are accepted via the `i64 as u32` cast and
  become enormous positive values that corrupt multiple fields.

## Reproduction (property-based test)

`prop_encode_sbfm_tests::prop_rejects_out_of_range_immediates` fails on the
**first** generated input:

```
minimal failing input: bad_immr = 64, bad_imms = 64, neg_imm = -3
panicked: immr=64 (>63) should be rejected, got Ok(Word(2470445088))
```

`Word(2470445088)` is `0x9340_0000`. Decoding:

```
0x93400000 = 1001 0011 0100 0000 0000 0000 0000 0000
 sf[31]=1  opc[30:29]=00  100110[28:23]  N[22]=1  immr[21:16]=000000 ...
```

The intended `immr=64` did **not** land in the `immr` field (which reads back as
0); it instead set bit 22, exactly the `N` bit position (`64 << 16 == 1 << 22`).
The encoder returned `Ok` with a word whose fields no longer correspond to the
inputs — a silent miscompile.

## Expected behavior

`encode_sbfm` should return `Err(...)` whenever `immr` or `imms` is outside the
6-bit range `[0, 63]` (and ideally also reject the additional width-dependent
`SBFM` constraints), matching the contract every other validated-field encoder
in the file is expected to uphold.

## Suggested fix

```rust
if immr > 63 || imms > 63 {
    return Err(format!("SBFM immr/imms must be 0..=63, got immr={} imms={}", immr, imms));
}
```

(`get_imm` already returns `i64`; the negative case should be rejected too, e.g.
by checking the `i64` value before the `as u32` cast.)

## Scope

The same `as u32`-no-validation pattern (and the same bug class) is present in
the sibling encoders in this file: `encode_ubfm` (already documented in
`prop_encode_ubfm_tests`), and very likely `encode_bfm`, `encode_sbfx`,
`encode_ubfx`, `encode_sbfiz`, `encode_ubfiz`, `encode_bfi`, `encode_bfxil`. A
shared validation helper for 6-bit bitfield immediates would address all of them.

## Test status (this campaign)

| Property | Result |
|---|---|
| `prop_sbfm_field_placement` (structural + N==sf) | PASS |
| `prop_sbfm_equals_sbfx_alias` (SBFX differential) | PASS |
| `prop_sbfm_xor_ubfm_is_only_bit_30` (UBFM differential) | PASS |
| `prop_width_changes_only_sf_and_n` (width differential) | PASS |
| `prop_rejects_out_of_range_immediates` (negative contract) | **FAIL — this bug** |
| `prop_rejects_malformed_operands` (negative contract) | PASS |
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/159
