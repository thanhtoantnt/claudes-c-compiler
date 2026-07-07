# Bug Report — `encode_ldur_stur` silently wraps out-of-range `imm9`

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldur_stur(operands, is_load, op2_bits)`
**Severity:** Medium (silent mis-assembly; wrong code emitted with no diagnostic)
**Found by:** property `prop_encode_ldur_stur_tests::prop_out_of_range_imm9_is_rejected` (failing)

## Summary

`encode_ldur_stur` encodes the AArch64 **LDUR/STUR/LDTR/STTR** (unscaled-immediate)
instructions. Per the ARMv8-A Architecture Reference Manual (§C4.1.66), the
`imm9` field is a **9-bit *signed*** immediate with encodable range
**[-256, +255]**.

The implementation does **not** range-check the offset. It blindly masks:

```rust
let imm9_enc = (imm9 as u32) & 0x1FF;
let word = (size << 30) | (0b111 << 27) | (v << 26) | (opc << 22)
    | (imm9_enc << 12) | (op2_bits << 10) | (rn << 5) | rt;
Ok(EncodeResult::Word(word))   // <- always Ok, never validates range
```

As a result, offsets outside [-256, 255] are **silently wrapped** to a
*different* in-range offset, producing incorrect machine code with no error.

## Minimal failing input (from proptest)

```
excess = 1, negative = false   ->   offset = 256
```

- Input:  `ldur x0, [x1, #256]`
- Got:    `Ok(Word(0xF8500020))` — i.e. the encoding of `ldur x0, [x1, #-256]`
- Expected: `Err(...)` (256 is outside the imm9 range [-256, 255]; the programmer should use the `LDR` unsigned-offset form, `ldr x0, [x1, #256]`).

## Why it's wrong — wrap table

`imm9_enc = (offset as i32 as u32) & 0x1FF`, reinterpreted as 9-bit signed:

| requested offset | imm9 field | reinterpreted as | emitted encoding is actually for |
|------------------|------------|------------------|-----------------------------------|
| `#256`           | `256`      | **-256**         | `ldur x0,[x1,#-256]` → `0xF8500020` |
| `#257`           | `257`      | **-255**         | `ldur x0,[x1,#-255]`              |
| `#511`           | `511`      | **-1**           | `ldur x0,[x1,#-1]`                |
| `#512`           | `0`        | **0**            | `ldur x0,[x1,#0]`                 |
| `#-257`          | `255`      | **+255**         | `ldur x0,[x1,#255]`               |
| `#-512`          | `0`        | **0**            | `ldur x0,[x1,#0]`                 |

Every entry silently mis-assembles. A negative offset of `-257` even flips
sign and becomes a *positive* `+255`.

## Contrast with the sibling function

`encode_ldr_str` (the LDR/STR path in the same file) **does** reject
out-of-range immediates (its negative-contract property
`prop_out_of_range_offset_is_rejected` passes). `encode_ldur_stur` is the
inconsistent, unsafe variant.

## Suggested fix

Validate the offset against the 9-bit signed range before encoding:

```rust
if imm9 < -256 || imm9 > 255 {
    return Err(format!(
        "ldur/stur: unscaled offset {} out of range [-256, 255] \
         (use the LDR/STR unsigned-offset form for larger offsets)",
        imm9
    ));
}
let imm9_enc = (imm9 as u32) & 0x1FF;
```

## Test evidence

```
cargo test --lib prop_encode_ldur_stur
  prop_gp_layout_matches_golden              ... ok
  prop_imm9_field_sign_extended_equals_input ... ok
  prop_size_field_tracks_reg_width           ... ok
  prop_load_xor_store_is_opc_bit22           ... ok
  prop_op2_bits_in_field_11_10               ... ok
  prop_out_of_range_imm9_is_rejected         ... FAILED   <-- this bug
```

The five passing properties independently confirm (via ARM-ARM golden
encodings) that field placement, size/opc derivation, the load⊕store opc bit,
and the `op2_bits` (LDUR=00 vs LDTR=10) field are all correct — the defect is
isolated to the missing range check on `imm9`.
