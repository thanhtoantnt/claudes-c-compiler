# Bug Report — `encode_ubfx` does not validate `lsb`/`width` immediates

**File:** `src/backend/arm/assembler/encoder/bitfield.rs`
**Function:** `encode_ubfx`
**Severity:** High (emits a corrupted AArch64 encoding for invalid input with no diagnostic)

## Summary

`encode_ubfx` accepts the `lsb` and `width` immediates via `get_imm(...) as u32`
with **no range validation**, then ORs them into the 32-bit encoding word. An
architecturally out-of-range immediate is therefore silently truncated instead
of being rejected with `Err`.

## Root cause

```rust
let lsb = get_imm(operands, 2)? as u32;   // no range check
let width = get_imm(operands, 3)? as u32; // no range check
let immr = lsb;
let imms = lsb + width - 1;
let word = (sf << 31) | ... | (immr << 16) | (imms << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))   // always Ok
```

`get_imm` (`encoder/mod.rs:968`) returns the raw `i64`; the `as u32` cast and
the `<< 16` / `<< 10` ORs carry out-of-range values into adjacent opcode
fields with no check.

## Minimal failing input

Assemble `UBFX w0, w1, #64, #1`.

- **Expected:** `Err` — for a 32-bit (`w`) register the ARM ARM Bitfield
  encoding requires `0 <= lsb <= 31`, so `lsb = 64` is invalid.
- **Actual:** `Ok(Word(0x5340_0820))` (1396768800) — the out-of-range `lsb`
  overflows the 6-bit `immr` field `[21:16]` into the `N` bit `[22]`,
  producing a valid-looking but wrong instruction.

## Evidence (property test)

The negative-contract property
`prop_encode_ubfx_tests::prop_rejects_out_of_range_immediates` asserts the
encoder returns `Err` for out-of-range immediates. It fails on its first
generated case:

```
minimal failing input: is_64 = false, bad_lsb = 64, bad_width = 65, neg_imm = -3
panicked: lsb=64 (>31) should be rejected, got Ok(Word(1396768800))
```

## Impact

Invalid assembler input silently produces a corrupted 32-bit opcode; the user
gets no diagnostic and downstream disassembly/simulation see a wrong
instruction. Same defect class as the sibling encoders `encode_ubfm`,
`encode_sbfm`, `encode_bfm` (all use the unchecked `get_imm(...) as u32`
pattern).

## Suggested fix

Validate `lsb`/`width` against the register width before encoding:

```rust
let regsize = if is_64 { 64u32 } else { 32u32 };
if lsb >= regsize {
    return Err(format!("UBFX: lsb {} out of range [0, {})", lsb, regsize));
}
if width == 0 || lsb + width > regsize {
    return Err(format!("UBFX: width {} out of range [1, {}]", width, regsize - lsb));
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/163
