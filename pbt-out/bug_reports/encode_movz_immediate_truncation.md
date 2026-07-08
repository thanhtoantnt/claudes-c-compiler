# Bug Report: `encode_movz` silently truncates out-of-range immediate magnitude

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movz`

## Summary

`encode_movz` masks the immediate with `& 0xFFFF` without first validating that the source immediate fits in the 16-bit MOVZ field. Out-of-range immediates are accepted and silently encoded as a different value.

## Reproduction

Failing property: `movz_rejects_out_of_range_immediate`

Minimal input:

```text
rd = 0, imm = 65536
```

`movz x0, #0x10000` encodes `imm16 = 0x10000 & 0xFFFF = 0x0000`, producing the same word as `movz x0, #0x0` instead of returning `Err`.

Relevant source:

```rust
let imm = get_imm(operands, 1)?;
...
let word = (sf << 31) | (0b10100101 << 23) | (hw << 21)
         | (((imm as u32) & 0xFFFF) << 5) | rd;
```

## Impact

Silent miscompilation: constants that appear to assemble cleanly execute with the wrong value, e.g. `0x10001` becomes `0x0001`.

## Suggested fix

Validate `imm` before encoding:

```rust
if !(0..=0xFFFF).contains(&imm) {
    return Err(format!("movz immediate out of range: {}", imm));
}
```

The same validation pattern should be applied to `encode_movk` and `encode_movn`, which duplicate the masking pattern.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/65
