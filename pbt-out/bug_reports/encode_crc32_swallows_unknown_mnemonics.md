# Bug Report: `encode_crc32` silently accepts unknown mnemonics

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_crc32`
**Severity:** Medium

## Summary

`encode_crc32` never validates `mnemonic` argument. Unknown strings treated as `crc32b` (sf=0, sz=00) via `_ => (0, 0b00)` catch-all. `contains("crc32c")` substring match sets C=1 for any token containing that substring.

## Root Cause

```rust
let is_c = mnemonic.contains("crc32c");            // substring match, not equality
let (sf, sz) = match mnemonic {
    "crc32b" | "crc32cb" => (0u32, 0b00u32),
    "crc32h" | "crc32ch" => (0, 0b01),
    "crc32w" | "crc32cw" => (0, 0b10),
    "crc32x" | "crc32cx" => (1, 0b11),
    _ => (0, 0b00),                                 // catch-all default → no error
};
```

## Reproduction

**Input:** `encode_crc32("crc32", &[wreg(0), wreg(1), wreg(2)])`

**Expected:** `Err` — unknown mnemonic

**Actual:** `Ok(Word(0x1AC24020))` — treated as crc32b

**Other failing inputs:** `"crc32d"`, `"crc32cz"`, `"foo"`, `""`

## Impact

Typos (`crc32d`, `crc32`, `nop`) emitted as valid-looking words with no diagnostic. Corrupted/unallocated encodings while reporting success.

## Suggested Fix

Reject unknown mnemonics:

```rust
let (is_c, sf, sz) = match mnemonic {
    "crc32b"  => (false, 0u32, 0b00u32),
    "crc32cb" => (true,  0u32, 0b00u32),
    "crc32h"  => (false, 0u32, 0b01u32),
    "crc32ch" => (true,  0u32, 0b01u32),
    "crc32w"  => (false, 0u32, 0b10u32),
    "crc32cw" => (true,  0u32, 0b10u32),
    "crc32x"  => (false, 1u32, 0b11u32),
    "crc32cx" => (true,  1u32, 0b11u32),
    _ => return Err(format!("unknown CRC32 mnemonic: {}", mnemonic)),
};
```

## Regression Property

Failing property: `prop_rejects_unknown_mnemonics`

```rust
prop_assert!(encode_crc32("crc32", &[wreg(0), wreg(1), wreg(2)]).is_err());
prop_assert!(encode_crc32("crc32d", &[wreg(0), wreg(1), wreg(2)]).is_err());
prop_assert!(encode_crc32("foo", &[wreg(0), wreg(1), wreg(2)]).is_err());
prop_assert!(encode_crc32("", &[wreg(0), wreg(1), wreg(2)]).is_err());
```

## PBT Results (module `prop_encode_crc32_tests`)

| Property | Result |
|---|---|
| `prop_crc32_field_placement` | PASS |
| `prop_mnemonic_drives_size_bits` | PASS |
| `prop_register_width_is_ignored` | PASS |
| `prop_rejects_malformed_operands` | PASS |
| `prop_rejects_unknown_mnemonics` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/143