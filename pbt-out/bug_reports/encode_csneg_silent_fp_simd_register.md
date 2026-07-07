# Bug Report — `encode_csneg` silently accepts FP/SIMD register operands

## Location
`src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_csneg`
(signature: `fn encode_csneg(operands: &[Operand]) -> Result<EncodeResult, String>`).

## Summary
`encode_csneg` decodes its three register operands (`Rd`, `Rn`, `Rm`) with the
shared helper `get_reg`, which calls `parse_reg_num`. That helper accepts *any*
register prefix in `{'x','w','d','s','q','v','h','b'}` and returns the trailing
number as the 5-bit register field. The width bit `sf` is then derived from
`is_64bit_reg`, which only recognises `x`/`sp`/`xzr`/`lr` as 64-bit.

Consequently a floating-point/SIMD register name such as `d0`, `s1`, `q2`, `v3`,
`h4`, or `b5` is **not rejected** — it is silently re-encoded as if it were a
general-purpose register with `sf = 0`, producing a malformed 32-bit CSNEG
instruction word. The AArch64 Architecture Reference Manual (C4.1.68,
"Conditional Select (negate)") defines CSNEG **only** on general-purpose (X/W)
registers, so every such emission is an unallocated encoding.

This is the same latent validation gap already documented for the sibling
conditional-select encoders (`encode_csel`, `encode_csinc`, `encode_csinv`).

## Root cause
```rust
pub(crate) fn encode_csneg(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // <- accepts "b0", "v3", ...
    let (rn, _) = get_reg(operands, 1)?;        // <- ditto
    let (rm, _) = get_reg(operands, 2)?;        // <- ditto
    ...
    let sf = sf_bit(is_64);                      // <- sf=0 for any FP/SIMD prefix
    let word = ((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12) | (0b01 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```
with `get_reg` (`encoder/mod.rs:956`) and `parse_reg_num` (`encoder/mod.rs:131`):
```rust
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```
`is_64bit_reg` / `is_fp_reg` exist in `encoder/mod.rs` but are never consulted by
the register-class validation path, so the operand is never classified as FP/SIMD.

## Reproduction (property test)
The committed property `prop_rejects_fp_simd_registers` (in module
`prop_encode_csneg_tests`) asserts the correct contract — that an FP/SIMD register
in any of the three register slots returns `Err`. Running it:

```
$ cargo test prop_encode_csneg
...
running 6 tests
test ...prop_opcode_structure_and_fields ... ok
test ...prop_csneg_xor_csinv_is_bit10 ... ok
test ...prop_sf_bit_is_bit31 ... ok
test ...prop_cond_round_trips_and_aliases ... ok
test ...prop_rejects_invalid_operands ... ok
test ...prop_rejects_fp_simd_registers ... FAILED

failures:
---- ...prop_rejects_fp_simd_registers stdout ----
Test failed: encode_csneg should reject FP/SIMD register in slot 0
  (got Ok(Word(1518470176)))
minimal failing input: prefix = "b", n = 0, slot = 0
```
`1518470176 == 0x5A80_0000` decodes to `sf=0 op=1 11010100 00000 0000 01 00000 00000`
— i.e. `CSNEG W0, W0, W0, EQ` (a 32-bit GP instruction), emitted from the FP/SIMD
mnemonic `b0`.

## Impact
- **Severity: Low.** `encode_csneg` is `pub(crate)` and reached only through the
  assembler's mnemonic dispatch table; hand-written assembly that passes an
  FP/SIMD register to `csneg` is rare. The malformed word would still be rejected
  downstream by a stricter disassembler/emulator, or silently mis-executed on
  real hardware (an UNALLOCATED encoding traps to an exception handler).
- **Correctness:** emits an UNALLOCATED AArch64 encoding for every FP/SIMD
  register operand, instead of failing fast at assembly time.
- **Consistency:** identical behaviour exists in `encode_csel`, `encode_csinc`,
  and `encode_csinv`; fixing `get_reg` (or adding a register-class guard in each
  conditional-select encoder) closes the whole family.

## Suggested fix
Reject non-GP registers in `get_reg`, or add an explicit guard at the top of
`encode_csneg` (and its siblings):
```rust
fn assert_gp(name: &str) -> Result<(), String> {
    if is_fp_reg(name) {
        return Err(format!("CSNEG requires a GP (X/W) register, got {}", name));
    }
    Ok(())
}
```
Alternatively, tighten `parse_reg_num` (or introduce a dedicated
`parse_gp_reg_num`) so FP/SIMD prefixes no longer return a value when a GP
register is required.

## Properties added (`prop_encode_csneg_tests`, 6 total)
| # | Property | Status |
|---|----------|--------|
| A | `prop_opcode_structure_and_fields` — full field-placement oracle | ✅ pass |
| B | `prop_csneg_xor_csinv_is_bit10` — CSNEG/CSINV differ only in bit 10 | ✅ pass |
| C | `prop_sf_bit_is_bit31` — 64- vs 32-bit Rd differ only in bit 31 | ✅ pass |
| D | `prop_cond_round_trips_and_aliases` — cond table + cs/hs, cc/lo aliases | ✅ pass |
| E | `prop_rejects_invalid_operands` — arity / type structural negative contract | ✅ pass |
| F | `prop_rejects_fp_simd_registers` — register-class negative contract | ❌ **fail (this bug)** |
