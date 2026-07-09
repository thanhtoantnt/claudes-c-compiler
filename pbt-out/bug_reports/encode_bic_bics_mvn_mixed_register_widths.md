# Bug Report: `encode_bic`/`encode_bics`/`encode_mvn` silently accept mixed register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_bic`, `encode_bics`, `encode_mvn`
**Severity:** High

## Summary

`encode_bic`, `encode_bics`, and `encode_mvn` derive the `sf` (operand-width) bit
**only from the destination register** and discard the width flags returned for the
source register operands. AArch64 *Logical (shifted register)* instructions
(ARMv8 ARM §C4.1.115) require `<Rd>`, `<Rn>` and `<Rm>` (and for MVN `<Rd>`/`<Rm>`)
to all share one width. Mixed-width forms such as `bic x0, w1, w2`, `bics x0, w1, w2`,
or `mvn x0, w1` are therefore silently accepted and emitted at the destination's
width, with the W register *numbers* placed into a 64-bit instruction. Reference
assemblers (GAS / `llvm-mc`) reject all of these with an "operand size mismatch"
error.

This is the **same defect class** already reported for `encode_eon`
(`encode_eon_mixed_register_widths.md`) and `encode_orn`
(`encode_orn_mixed_register_widths.md`); this report covers the **three remaining**
logical-NOT encoders that were not previously reported.

## Root Cause

In all three functions `get_reg` returns `(num, is_64)` but every `is_64` except the
destination's is bound to `_` and dropped:

```rust
// encode_bic / encode_bics:
let (rd, is_64) = get_reg(operands, 0)?;   // destination width KEPT
let (rn, _) = get_reg(operands, 1)?;       // Rn width DISCARDED
...
let (rm, _) = get_reg(operands, 2)?;       // Rm width DISCARDED (parse_reg_num path)
let sf = sf_bit(is_64);                    // sf from destination only

// encode_mvn:
let (rd, is_64) = get_reg(operands, 0)?;   // destination width KEPT
let (rm, _) = get_reg(operands, 1)?;       // Rm width DISCARDED
let sf = sf_bit(is_64);
```

## Reproduction

| Source text           | Expected | Actual (`Ok`)           | `sf` (bit 31) |
|-----------------------|----------|-------------------------|---------------|
| `bic x0, w1, w2`      | `Err`    | `Ok(0x8A220020)`        | 1 (64-bit)    |
| `bics x0, w1, w2`     | `Err`    | `Ok(0xEA220020)`        | 1 (64-bit)    |
| `mvn x0, w1`          | `Err`    | `Ok(0xAA2103E0)`        | 1 (64-bit)    |
| `mvn w0, x1`          | `Err`    | `Ok(0x2A2103E0)`        | 0 (32-bit)    |

In every case the encoded instruction operates at the **destination's** width while
the source text names the opposite width. For `mvn w0, x1` the encoder produces a
32-bit MVN that reads `x1`'s low 32 bits — silently dropping the programmer's
intent.

## Impact

A width-mismatch typo in hand-written (or compiler-generated) assembly assembles
without any diagnostic into an instruction operating at a different width than the
source text names. This can corrupt results (e.g. `mvn w0, x1` discards the high
32 bits the programmer expected to read) and is extremely hard to debug because the
emitted encoding is *valid* — it simply differs from what was written.

## Suggested Fix

Compare the `is_64` flags returned by `get_reg` for all operands and reject on
mismatch:

```rust
// encode_bic / encode_bics:
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_64) = get_reg(operands, 1)?;
// ... (rm path:)
if rn_64 != is_64 || rm_64 != is_64 {
    return Err("operand size mismatch: all operands must share the same width".into());
}

// encode_mvn:
let (rd, is_64) = get_reg(operands, 0)?;
let (rm, rm_64) = get_reg(operands, 1)?;
if rm_64 != is_64 {
    return Err("operand size mismatch: mvn operands must share the same width".into());
}
```

## Regression Property

Failing properties: `bic_rejects_mixed_register_widths`,
`bics_rejects_mixed_register_widths`, `mvn_rejects_mixed_register_widths`
(in `data_processing_logical_not_pbt.rs`, marked `#[ignore]`).

```rust
prop_assert!(encode_bic(&[xreg(0), wreg(1), wreg(2)]).is_err());   // bic x0,w1,w2
prop_assert!(encode_bics(&[xreg(0), wreg(1), wreg(2)]).is_err());  // bics x0,w1,w2
prop_assert!(encode_mvn(&[xreg(0), wreg(1)]).is_err());            // mvn x0,w1
prop_assert!(encode_mvn(&[wreg(0), xreg(1)]).is_err());            // mvn w0,x1
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/266
