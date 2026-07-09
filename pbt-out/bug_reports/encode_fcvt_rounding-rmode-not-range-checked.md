# B1 — `encode_fcvt_rounding`: `rmode` not range-checked (silent overflow)

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fcvt_rounding(operands, rmode, opcode)`
**Witness test:** `prop_fcvt_rounding_rejects_oversized_rmode`
(`src/backend/arm/assembler/encoder/fp_scalar_fcvt_rounding_pbt.rs`, `#[ignore]`)
**Reproduce:** `cargo test prop_fcvt_rounding_rejects_oversized_rmode -- --ignored`

## Minimal input

```rust
encode_fcvt_rounding(&[Operand::Reg("w0".into()), Operand::Reg("s0".into())],
                     /* rmode */ 4, /* opcode */ 0)
```

## Expected vs actual

- **Expected:** `Err` — `rmode` is a **2-bit** field at bits `[20:19]`
  (valid range 0..=3), so 4 is out of range.
- **Actual:** `Ok(0x1E380000)`-class word that is silently corrupted.

## Root cause

```rust
let word = ((sf << 31) | (0b11110 << 24) | (ftype << 22)
    | (1 << 21) | (rmode << 19) | (opcode << 16)) | (rn << 5) | rd;
```

`rmode` is OR'd in with no range check. `rmode = 4` ⇒ `(4 << 19) == (1 << 21)`,
colliding with the fixed bit 21 that marks the FCVT* conversion class; larger
values also corrupt `ftype` at `[23:22]`.

## Impact

The encoder emits a 32-bit word that is **not a valid FCVT\* encoding** yet
returns `Ok`, so the assembler will silently assemble an UNDEFINED instruction.
Property test first fails on `rmode = 4`.

## Fix

```rust
if rmode > 0b11 {
    return Err(format!("rmode {} out of range (0..=3)", rmode));
}
```
