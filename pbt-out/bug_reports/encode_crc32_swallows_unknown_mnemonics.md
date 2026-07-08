# Bug Report — `encode_crc32` silently accepts unknown mnemonics

## Location
`src/backend/arm/assembler/encoder/bitfield.rs`, function `encode_crc32(mnemonic, operands)`.

## Summary
The CRC32 encoder never validates the `mnemonic` argument. Any string that is
not one of the eight canonical CRC32 mnemonics (`crc32{b,h,w,x}`,
`crc32c{b,h,w,x}`) is silently accepted and encoded as if it were a valid
instruction, instead of returning `Err`.

## Root cause
Two pieces of logic together swallow the error:

```rust
let is_c = mnemonic.contains("crc32c");            // substring match, not equality
let c_bit = if is_c { 1u32 } else { 0 };

let (sf, sz) = match mnemonic {
    "crc32b" | "crc32cb" => (0u32, 0b00u32),
    "crc32h" | "crc32ch" => (0, 0b01),
    "crc32w" | "crc32cw" => (0, 0b10),
    "crc32x" | "crc32cx" => (1, 0b11),
    _ => (0, 0b00),                                 // catch-all default → no error
};
```

- The `match` has a `_ => (0, 0b00)` catch-all, so an unrecognized mnemonic is
  treated as `crc32b` (sf=0, sz=00).
- `is_c` uses `contains("crc32c")`, so any token containing that substring
  (e.g. `"crc32cz"`, `"my_crc32c_thing"`) gets `C=1` regardless of validity.

## Impact
- A typo or parser slip (`crc32d`, `crc32y`, `crc32` with no size, `"crc32c"`
  with no size, `"nop"`, `""`, …) is emitted as a *valid-looking* 32-bit word
  with no diagnostic. The assembler produces a corrupted/unallocated encoding
  while reporting success.
- The C-bit substring heuristic makes this worse: `"crc32cz"` encodes as a
  CRC32C byte op (`C=1, sz=00`) rather than being rejected.

## Reproduction
```text
encode_crc32("crc32",   &[Reg("w0"), Reg("w1"), Reg("w2")]) == Ok(Word(0x1AC24020))  // bogus
encode_crc32("crc32d",  &[Reg("w0"), Reg("w1"), Reg("w2")]) == Ok(Word(...))         // bogus
encode_crc32("crc32cz", &[Reg("w0"), Reg("w1"), Reg("w2")]) == Ok(Word(...))         // C=1, bogus
encode_crc32("foo",     &[Reg("w0"), Reg("w1"), Reg("w2")]) == Ok(Word(...))         // bogus
encode_crc32("",        &[Reg("w0"), Reg("w1"), Reg("w2")]) == Ok(Word(...))         // bogus
```
All return `Ok`; each should return `Err`.

## Expected behavior
Return `Err` for any `mnemonic` not in the exact set
`{crc32b, crc32h, crc32w, crc32x, crc32cb, crc32ch, crc32cw, crc32cx}`.
Recommended fix: drop the `_` arm (or have it `return Err(...)`) and replace
the `contains("crc32c")` heuristic with an explicit per-mnemonic lookup.

## Test evidence
Property suite added in `mod prop_encode_crc32_tests` (same file):

| Property | Result |
|---|---|
| `prop_crc32_field_placement` (field/bit-layout oracle) | PASS |
| `prop_mnemonic_drives_size_bits` (sf/sz/C from mnemonic) | PASS |
| `prop_register_width_is_ignored` (documents width not validated) | PASS |
| `prop_rejects_malformed_operands` (missing/wrong-type operands → Err) | PASS |
| `prop_rejects_unknown_mnemonics` (negative contract) | **FAILS — confirms bug** |

## Secondary observation (not a hard bug, low severity)
The encoder discards register width entirely (`let (rd, _) = get_reg(...)`),
so `crc32x w0, w1, w2` encodes bit-identically to `crc32x x0, x1, x2` (sf=1).
AArch64 requires `crc32x`/`crc32cx` to take `Xd` and `Xm` (with `Wn`), so
accepting `w`-registers here is architecturally inconsistent. Documented by
`prop_register_width_is_ignored`.
