# BUG: `encode_ubfm` silently accepts out-of-range `immr`/`imms` immediates

**File:** `src/backend/arm/assembler/encoder/bitfield.rs` — `encode_ubfm`
**Severity:** High (produces malformed / architecturally-UNDEFINED machine code instead of an error)
**Found by:** property-based test suite `prop_encode_ubfm_tests`

## Summary

`encode_ubfm` casts the `#immr` and `#imms` operands straight to `u32` via `as u32`
and ORs them into the instruction word with **no range validation and no masking**:

```rust
pub(crate) fn encode_ubfm(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let immr = get_imm(operands, 2)? as u32;   // ← no 0..63 check
    let imms = get_imm(operands, 3)? as u32;   // ← no 0..63 check
    let sf = sf_bit(is_64);
    let n = if is_64 { 1u32 } else { 0u32 };
    let word = (sf << 31) | (0b10 << 29) | (0b100110 << 23) | (n << 22)
             | (immr << 16) | (imms << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Why this is a bug

In the AArch64 **UBFM** encoding (ARM ARM, Bitfield — `sf 10 100110 N immr imms Rn Rd`):

* `immr` occupies bits **[21:16]** — a 6-bit field, valid range **0–63**.
* `imms` occupies bits **[15:10]** — a 6-bit field, valid range **0–63**.
* `N` at bit **[22]** must equal `sf` (architectural CONSTRAINT).

Because the encoder neither validates nor masks the immediates:

| Input                              | Effect on the encoded word                                                                                          |
|------------------------------------|---------------------------------------------------------------------------------------------------------------------|
| `immr = 64`                        | `64 << 16 == 1 << 22` → overwrites the **N** bit (and the sf/N invariant).                                          |
| `immr = 128`                       | `128 << 16 == 1 << 23` → overwrites the fixed `100110` opcode bits → different instruction entirely.                |
| `immr = 256`                       | `256 << 16 == 1 << 24` → corrupts opcode → emits a **different, UNDEFINED** instruction.                            |
| `imms >= 64`                       | Collides with `Rn [9:5]` and beyond, silently mutating the source register and opcode.                              |
| negative immediate (`immr = -1`)   | `(-1i64) as u32 == 0xFFFFFFFF`; `<< 16` turns the whole upper word into garbage (`0xFFFF0000`).                      |

A real assembler (`llvm-mc`, GNU `as`) rejects these with an error, e.g.:

```
ubfm x0, x1, #64, #0     →  error: immediate must be an integer in range [0, 63].
```

The current code instead emits `Ok(Word(...))` and produces invalid object code,
which will be disassembled as an unrelated instruction or trap as UNDEFINED at
runtime.

## Minimal failing case (proptest)

```
input:  UBFM x0, x1, #64, #0
        (bad_immr = 64, bad_imms = 64, neg_imm = -3)

expected: Err(...)
actual:   Ok(Word(3544186912))   // 0xD3400020 — opcode bits corrupted
```

`prop_rejects_out_of_range_immediates` fails on this input.

## Properties written (6 total, 1 failing)

| # | Property | Status | Oracle |
|---|----------|--------|--------|
| A | `prop_ubfm_field_placement` | ✅ pass | structural (all fixed bits + fields for valid 0..63 inputs) |
| B | `prop_n_equals_sf` | ✅ pass | invariant (N == sf, ARM ARM CONSTRAINT) |
| C | `prop_width_changes_only_sf_and_n` | ✅ pass | differential (x vs w differ only in sf[31] & N[22]) |
| D | `prop_deterministic` | ✅ pass | purity |
| E | `prop_rejects_out_of_range_immediates` | ❌ **FAIL** | negative contract (immr/imms must be 0..63) |
| F | `prop_rejects_malformed_operands` | ✅ pass | negative contract (missing / wrong-typed operands → Err) |

The same latent defect exists in the sibling raw encoders `encode_sbfm`,
`encode_bfm`, and (via the same `as u32` pattern) the alias encoders
`encode_ubfx`, `encode_sbfx`, `encode_bfxil`, `encode_sbfiz`, `encode_ubfiz`,
`encode_bfi`, `encode_extr`.

## Suggested fix

Validate the immediate fields against the architectural size before encoding,
returning `Err` on violation:

```rust
let (immr, imms) = {
    let ir = get_imm(operands, 2)?;
    let is = get_imm(operands, 3)?;
    let max = if is_64 { 63 } else { 31 };
    if !(0..=max).contains(&ir) {
        return Err(format!("immr out of range [0,{}]: {}", max, ir));
    }
    if !(0..=max).contains(&is) {
        return Err(format!("imms out of range [0,{}]: {}", max, is));
    }
    (ir as u32, is as u32)
};
```

(At minimum, mask with `& 0x3F` — but rejection is correct, since wrapping is
*not* intentional here per the ARM ARM.)
