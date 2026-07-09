# Bug report — `encode_neon_float_two_misc` silently overflows `u_bit` / `opcode` / `size_hi`

**File:** `src/backend/arm/assembler/encoder/neon.rs`
**Function:** `encode_neon_float_two_misc(operands, u_bit, size_hi, opcode)`
**Encoding group:** AArch64 "Advanced SIMD two-register miscellaneous (floating-point)"
layout `0 Q U 01110 size 10000 opcode 10 Rn Rd` (FABS, FNEG, FCVTZS/ZU, SCVTF, UCVTF, FRINT*, ...).

## Summary

The encoder performs **no range validation** on its three numeric parameters
`u_bit`, `size_hi`, and `opcode`. Out-of-range values are not rejected; they are
shifted into the 32-bit word and silently **overflow into adjacent fields and
the constant `01110`/`10000` fields**, producing a corrupt instruction word
that does not correspond to any defined AArch64 instruction.

This is the same defect class already documented in `neon_two_misc_pbt.rs`
(`encode_neon_two_misc`), but here it additionally covers the `size_hi`
parameter, which is unique to the float variant.

## What passes (correct behavior)

The PBT suite in `src/backend/arm/assembler/encoder/neon_float_two_misc_pbt.rs`
confirms that for **all valid inputs** the encoder is bit-exact:

- `prop_float_two_misc_matches_arm_layout` — output equals an independently
  rebuilt reference word for `rd,rn ∈ [0,31]`, `arr ∈ {2s,4s,2d}`,
  `u_bit ∈ {0,1}`, `size_hi ∈ {0,1}`, `opcode ∈ [0,0x1F]`.
- `prop_float_two_misc_fields_isolated` — Rd∈[4:0], Rn∈[9:5], opcode∈[16:12],
  size∈[23:22]; no leakage between fields.
- `prop_float_two_misc_fixed_bits_and_inputs` — fixed bits constant; Q/U/size
  driven by arrangement/parameters.
- Golden values pinned to the real ISA: `fabs v0.2s`=0x0E20F800,
  `fabs v0.4s`=0x4E20F800, `fabs v0.2d`=0x4E60F800, `fneg v0.2s`=0x0E217800,
  `fcvtzs v5.4s,v7.4s`=0x4E21B8E5.
- Operand-count and arrangement restrictions are correctly enforced (only
  `2s`/`4s`/`2d` accepted; `8b`/`16b`/`4h`/`8h`/`1d`/garbage → `Err`).
- Register range **is** validated — indirectly: `parse_reg_num` returns `None`
  for num > 31, so `v32`/`v99` are correctly rejected (verified by
  `rejects_out_of_range_register`).

## The defect (failing negative-contract property)

`prop_float_two_misc_rejects_out_of_range_params` — **EXPECTED TO FAIL** —
minimized counterexample:

```
rd = 0, rn = 0, arr = "2s", bad_u = 2, bad_opcode = 32, bad_size_hi = 2
```

Each out-of-range parameter is accepted and corrupts the word:

| Parameter  | Valid range | Overflow target                                           | Example corruption                                          |
|------------|-------------|-----------------------------------------------------------|-------------------------------------------------------------|
| `u_bit`    | `{0,1}`     | bit 29 only; `u_bit≥2` hits bit 30 (Q) and bit 31          | `u_bit=2` → `fabs v0.2s` yields `0x4E20F800` (Q wrongly set)|
| `opcode`   | `[0,0x1F]`  | bits 16-12; `opcode≥0x20` hits bit 17 of the `10000` const | `opcode=0x20` (32) sets bit 17, breaking the constant       |
| `size_hi`  | `{0,1}`     | bit 23 (hi of size); `size_hi≥2` (`size_hi<<1≥2`) hits bit 24 of the `01110` const | `size_hi=2` → `size=4`, bit 24 set, breaking `01110` |

## Severity

A caller that passes an out-of-range value (e.g. a typo in a dispatch table,
or a future instruction variant whose opcode/size_hi exceeds the field width)
gets back `Ok(EncodeResult::Word(..))` with a silently-wrong instruction.
There is no signal of corruption; downstream this either assembles garbage or
is masked by a relocation/fixup, producing a hard-to-trace miscompilation.

## Suggested fix

Validate the three parameters up front and reject out-of-range values with
`Err`, mirroring the (already-correct) arrangement validation:

```rust
if u_bit > 1     { return Err(format!("float two-misc: u_bit {} out of range (0..=1)", u_bit)); }
if size_hi > 1   { return Err(format!("float two-misc: size_hi {} out of range (0..=1)", size_hi)); }
if opcode > 0x1F { return Err(format!("float two-misc: opcode {} out of range (0..=0x1F)", opcode)); }
```

With these guards the negative-contract property flips from failing to
passing, and the other seven properties remain green.

## Related lower-severity note

The source register's arrangement is read then discarded (bound to `_`); only
the destination arrangement drives Q/sz. ARM requires dest and source
arrangements to match, so a mismatch such as `Vd.4s, Vn.2d` is silently
accepted. (Analogous to the documented behavior in `encode_neon_two_misc`.)
The encoder trusts its caller here; lower priority than the overflow defect.
