# B2 — `encode_fcvt_rounding`: `opcode` not range-checked (silent overflow)

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fcvt_rounding(operands, rmode, opcode)`
**Witness test:** `prop_fcvt_rounding_rejects_oversized_opcode`
(`src/backend/arm/assembler/encoder/fp_scalar_fcvt_rounding_pbt.rs`, `#[ignore]`)
**Reproduce:** `cargo test prop_fcvt_rounding_rejects_oversized_opcode -- --ignored`

## Minimal input

```rust
encode_fcvt_rounding(&[Operand::Reg("w0".into()), Operand::Reg("s0".into())],
                     /* rmode */ 0, /* opcode */ 8)
```

## Expected vs actual

- **Expected:** `Err` — `opcode` is a **3-bit** field at bits `[18:16]`
  (valid range 0..=7), so 8 is out of range.
- **Actual:** `Ok(...)` word that is silently corrupted.

## Root cause

```rust
let word = ((sf << 31) | (0b11110 << 24) | (ftype << 22)
    | (1 << 21) | (rmode << 19) | (opcode << 16)) | (rn << 5) | rd;
```

`opcode` is OR'd in with no range check. `opcode = 8` ⇒ `(8 << 16) == (1 << 19)`
overflows into the adjacent **rmode** field at `[20:19]`; larger values corrupt
both rmode and the fixed bit 21.

## Impact

The encoder emits a 32-bit word that is **not a valid FCVT\* encoding** yet
returns `Ok`; the assembler silently assembles an UNDEFINED instruction. Property
test first fails on `opcode = 8`.

## Fix

```rust
if opcode > 0b111 {
    return Err(format!("opcode {} out of range (0..=7)", opcode));
}
```
