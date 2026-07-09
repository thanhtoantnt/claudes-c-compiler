# Bug — `encode_fp_1src`: `opcode` never range-checked, silently corrupts encoding

## Target
`pub(crate) fn encode_fp_1src(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/fp_scalar.rs`

Encodes AArch64 scalar FP data-processing (1 source): FRINTN/P/M/Z/A/X/I.

Verified-correct ARMv8-A field layout (reference oracle properties PASS for
in-range inputs, matching the caller's hard-coded opcodes `0b001000`…`0b001111`):

```
0  00  11110  ftype  1  opcode  10000  Rn  Rd
31           24 23:22 21  20:15  14:10  9:5 4:0
```

## Bug
`opcode` is a bare `u32` OR'd into a **6-bit** field at `[20:15]` with no
validation:

```rust
let word = (0b00011110u32 << 24) | (ftype << 22) | (1 << 21)
    | (opcode << 15) | (0b10000 << 10) | (rn << 5) | rd;
```

`opcode << 15` shifts bits past the field:
- bit 6 → bit 21 (already 1 ⇒ opcode **64 silently aliases to 0**),
- bit 7 → bit 22 = **ftype low bit**,
- bit 8 → bit 23 = **ftype high bit**.

Concrete corruption (single-precision dest, `ftype` should stay `00`):

| opcode | word produced | ftype[23:22] | effect                              |
|-------:|---------------|--------------|-------------------------------------|
|   8    | `0x1E244000`  | `00` (ok)    | correct FRINTN                      |
|  64    | `0x1E204000`  | `00`         | opcode silently aliased to 0        |
| 128    | `0x1E604000`  | `01`         | single dest rewritten as double     |

Latent today (callers pass only valid constants), but the public-ish signature
accepts any `u32`, so any future caller or typo emits wrong machine code with no
`Err`. This is the classic encoder "silent masking/overflow" anti-pattern.

## Reproduction
Failing property test in `fp_scalar::tests`:
`prop_fp_1src_rejects_out_of_range_opcode` — minimal input `bad = 64`.
```
cargo test prop_fp_1src_rejects_out_of_range_opcode
```

## Suggested fix
Reject out-of-range opcodes before encoding:
```rust
if opcode > 0x3F {
    return Err(format!("fp 1-source opcode out of range: {}", opcode));
}
```
(Tighter: only `{8,9,10,11,12,14,15}` are allocated FRINT* opcodes.)

## Severity
High (silent wrong-code emission on any out-of-range `opcode`).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/129
