# BUG: `encode_tbz` / `encode_tbnz` silently truncate out-of-range `bit` immediate

**File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
**Function:** `encode_tbz(operands, is_nz)` (shared by TBZ and TBNZ)
**Severity:** Medium — produces a *different, architecturally incorrect* instruction with no diagnostic.

## Summary

The test-bit immediate of TBZ/TBNZ is a 6-bit **unsigned** field `b5:b40`
(ARM ARM, C5.6.27 TBZ / C5.6.28 TBNZ), valid only in the range **0..=63**. No
spec permits wrapping or truncation. The encoder, however, never validates the
range and silently masks the immediate into the 6-bit field:

```rust
pub(crate) fn encode_tbz(operands: &[Operand], is_nz: bool) -> Result<EncodeResult, String> {
    let (rt, _) = get_reg(operands, 0)?;
    let bit = get_imm(operands, 1)?;            // i64, NO range check
    let (sym, addend) = get_symbol(operands, 2)?;
    let b5 = ((bit as u32) >> 5) & 1;            // <-- `bit as u32` truncates negatives
    let b40 = (bit as u32) & 0x1F;              // <-- silent masking to 6 bits
    ...
}
```

`get_imm` returns the raw `i64` with no validation
(`src/backend/arm/assembler/encoder/mod.rs:968`).

## Reproduction (PBT)

Two new properties in `compare_branch.rs::prop_encode_tbz_tests`:

- `prop_rejects_bit_above_63` — `bit` in 64..=4096 must be `Err`.
- `prop_rejects_negative_bit` — `bit` in -4096..=-1 must be `Err`.

Both **FAIL** with the minimal cases below.

## Minimal failing cases

```
tbz  w0, #64, target   -> word 0x36000000   (= tbz w0, #0, target)   ❌ should be Err
tbnz w0, #-2, target   -> word ... (b5=1,b40=30 -> bit 62)          ❌ should be Err
```

Concretely from the test run:

- `bit = 64`  -> `Ok(WordWithReloc { word: 905969664 (0x36000000), ... })`
  - `b5 = (64>>5)&1 = 0`, `b40 = 64 & 0x1F = 0`  ⇒ encodes as **bit 0**
- `bit = -2`  -> `Ok(...)` encoding **bit 62**
  - `(-2i64) as u32 = 0xFFFFFFFE`; `(0xFFFFFFFE >> 5) & 1 = 1`,
    `0xFFFFFFFE & 0x1F = 30`  ⇒ encodes as **bit (1<<5)|30 = 62**

So a typo or constant-folded `#64`/`#-2` is silently turned into a *valid but
unrelated* test of bit 0 / bit 62, with no error or warning. A different
instruction is emitted than the source requested.

## Differential check (real assemblers)

Both GNU `as` and `llvm-mc` reject these inputs:

```
$ echo 'tbz w0, #64, t' | llvm-mc --triple=aarch64 -show-encoding
error: expected compatible register or immediate
tbz w0, #64, t        # "immediate value out of range"

$ echo 'tbz w0, #-2, t' | llvm-mc --triple=aarch64 -show-encoding
error: ...
```

## Suggested fix

Validate `bit` after `get_imm` and reject out-of-range values before masking:

```rust
let bit = get_imm(operands, 1)?;
if !(0..=63).contains(&bit) {
    return Err(format!("tbz/tbnz: bit position {} out of range (valid 0..=63)", bit));
}
```

(Optionally also enforce that for a W (32-bit) register `bit <= 31` — a W
register with `b5 = 1` is CONSTRAINED UNPREDICTABLE per the ARM ARM and is
likewise rejected by `as`/`llvm-mc`. The current `prop_width_independent`
property asserts x{N}/w{N} encode identically for `bit` up to 63, which encodes
this second latent bug; out of scope for this report but worth a follow-up.)

## Status of the property suite

- Existing properties A–E (opcode structure, TBZ⊕TBNZ differential, width
  independence, bit round-trip, TstBr14 relocation) all **PASS** — they cover
  the valid range and confirm correct field placement there.
- New properties F (`prop_rejects_bit_above_63`) and G
  (`prop_rejects_negative_bit`) **FAIL**, demonstrating the missing range check.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/102
