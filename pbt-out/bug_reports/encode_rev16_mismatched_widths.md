# Bug Report — `encode_rev16` accepts mismatched Rd/Rn register widths

## Target
`encode_rev16` in `src/backend/arm/assembler/encoder/bitfield.rs`

```rust
pub(crate) fn encode_rev16(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;          // <-- width of Rn discarded
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (0b000001 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Summary
The encoder derives the `sf` (size) bit **only** from the destination register
`Rd` (operand 0) and throws away the width of the source register `Rn`
(operand 1) — `let (rn, _) = get_reg(operands, 1)?;`. As a result it silently
accepts architecturally invalid instructions whose two operands have different
widths, e.g. `REV16 w0, x1` and `REV16 x0, w1`.

## Spec basis (ARM ARM, Data-processing — 1 source)
The REV16 encoding carries a **single** `sf` bit:

```
sf 1 0 11010110 00000 opc[15:10]=000001 Rn Rd
```

Because there is exactly one size field, the ARM ARM **requires Rd and Rn to
share the same register width**. A mismatched-width pair (e.g. `REV16 Xd, Wn`)
is **UNPREDICTABLE / UNALLOCATED** and a correct assembler **must reject it**.

## Impact
- Severity: low–medium (no crash, no memory-unsafety).
- An emitter/assembler built on this encoder produces a valid-looking 32-bit
  word for an invalid instruction, silently masking user error. The emitted
  word's `Rn` field then names a register of the wrong width.
- Same class of bug already documented in this file for `encode_rev`
  (`prop_encode_rev_tests::prop_rejects_mismatched_widths`).

## Evidence (property-based test)
`prop_encode_rev16_tests::prop_rejects_mismatched_widths` (EXPECTED TO FAIL):

```
Test failed: REV16 with mismatched Rd/Rn widths (w0,x0) should be rejected,
got Ok(Word(1522533376))   # 1522533376 == 0x5AC0_0400
minimal failing input: rd = 0, rn = 0, rd_is_64 = false
```

`0x5AC0_0400` is the well-formed 32-bit `REV16 w0, w0` encoding, so the
mismatched source (`x0`) is silently coerced into a 32-bit instruction.

Full suite result for the target:
- PASS: field placement, ARM reference oracle (`0x5AC0_0400`/`0xDAC00400`),
  width-differential (only `sf` differs), REV-sibling differential (XOR
  confined to opc[15:10]), malformed-operand rejection, determinism.
- FAIL: mismatched-width rejection (this finding).

## Suggested fix
Validate that both operands share the same width before encoding, mirroring
the ARM ARM's `sf`-consistency constraint:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
if rd_is_64 != rn_is_64 {
    return Err(format!(
        "REV16: Rd and Rn must have the same width (got {}, {})",
        /*rd_name*/, /*rn_name*/
    ));
}
let sf = sf_bit(rd_is_64);
```

(Apply the same fix to the sibling `encode_rev` / `encode_rev32` scalar path,
which exhibit the identical bug.)
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/151
