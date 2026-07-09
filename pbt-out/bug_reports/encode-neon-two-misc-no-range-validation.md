# PBT Campaign Report — `encode_neon_two_misc`

## Summary

Property-based tested the AArch64 "Advanced SIMD two-register miscellaneous"
integer encoder (`encode_neon_two_misc`) in
`src/backend/arm/assembler/encoder/neon.rs`. Wrote 5 proptest properties + 3
deterministic golden/boundary checks. 4 properties and 3 checks **pass**; 1
**negative-contract** property **fails by design**, surfacing a real finding:
the encoder performs no range validation on its `u_bit` and `opcode`
parameters, so out-of-range values silently overflow into adjacent fixed/reserved
instruction fields and emit a corrupt but apparently-valid `EncodeResult::Word`.

## Modules Tested

| Module | File | Function | Oracle type |
|--------|------|----------|-------------|
| NEON two-register misc (integer) | `src/backend/arm/assembler/encoder/neon.rs` | `encode_neon_two_misc(operands, u_bit, opcode)` | reference (documented bit layout) + negative/error contract |

## Bugs Found

### BUG-1 — `encode_neon_two_misc`: no range validation on `u_bit` / `opcode` (silent field overflow)

**Severity:** correctness (latent; no current call site triggers it).

`encode_neon_two_misc` packs parameters into the 32-bit word with no width
checks:

```rust
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
    | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
Ok(EncodeResult::Word(word))
```

Per the encoding `0 Q U 01110 size 10000 opcode 10 Rn Rd`, `U` (bit 29) must be
0/1 and `opcode` (bits 16-12) must be a 5-bit value (0..=0x1F). Out-of-range
inputs are accepted and corrupt the word:

| call | produced word | what broke |
|------|---------------|------------|
| `u_bit=2, opcode=0` (`v0.8b,v0.8b`) | `0x4E200800` | `(2<<29)` sets **bit 30 (Q)**; for `.8b` Q should be 0 |
| `u_bit=4, opcode=0` | `0x8E200800` | sets **bit 31**, which the encoding requires to be a constant `0` |
| `opcode=32` | `0x0E220800` | `(32<<12)` sets bit 17, overwriting the constant `10000` field |
| `opcode=1024` | corrupt | sets bit 22, colliding with the `size` field |

Reproduction: failing property
`prop_two_misc_rejects_out_of_range_u_bit_and_opcode`, minimal input
`rd=0, rn=0, arr="8b", bad_u=2, bad_opcode=32`.

**Blast radius today:** all current call sites in `encoder/mod.rs` (ABS, NEG,
CLS/CLZ, SUQADD/USQADD, SADDLP/UADDLP, SADALP/UADALP) pass well-formed
`u_bit ∈ {0,1}` and `opcode ≤ 0b01011`, so live code is unaffected. The risk is
future instructions / refactorings passing an off-by-one opcode or miscomputed
signedness flag — they will emit a corrupt word silently.

**Fix:**
```rust
if u_bit > 1 {
    return Err(format!("two-register-misc: u_bit {} out of range (must be 0 or 1)", u_bit));
}
if opcode > 0x1F {
    return Err(format!("two-register-misc: opcode 0x{:x} out of range (must be 5 bits)", opcode));
}
```
Sibling `encode_neon_two_misc_narrow` shares the same unchecked-`u_bit`/`opcode`
pattern and is worth auditing together.

### Related finding (documented, not failing)

`prop_two_misc_source_arrangement_ignored` confirms the source register's
arrangement is read then discarded (bound to `_`). ARM requires dest and source
arrangements to match, so e.g. `Vd.4s, Vn.2d` is silently accepted with only the
destination arrangement driving Q/size. A consistency check would be a
worthwhile follow-up.

## Design Caveats

- **No `llvm-mc` / `aarch64-as` available** in this environment (`which` found
  only `objdump`). The reference oracle is therefore the **documented ARMv8 ARM
  bit layout** (`0 Q U 01110 size 10000 opcode 10 Rn Rd`), implemented
  independently of the SUT in `ref_word()` (it does not call
  `neon_arr_to_q_size`); golden words are derived by hand from that layout.
  Where an `as`/`llvm-mc` toolchain is available, a differential property
  against it would strengthen the suite.
- **Register numbers** are bounded by `parse_reg_num` (≤31) upstream, so the
  suite generates `0..=31`; it does not test `parse_reg_num` rejection itself.
- **Negative-contract property is expected to fail** until the fix lands; once
  fixed, flip the assertion to `is_ok()`/keep as a regression guard.
- Coverage is limited to the integer two-register-misc path; float
  (`encode_neon_float_two_misc`) and narrow
  (`encode_neon_two_misc_narrow`) variants are out of scope.

## Test Files Created

| File | Purpose |
|------|---------|
| `src/backend/arm/assembler/encoder/neon_two_misc_pbt.rs` | 5 proptest properties + 3 deterministic golden/boundary checks for `encode_neon_two_misc` |

Registered as `#[cfg(test)] mod neon_two_misc_pbt;` in
`src/backend/arm/assembler/encoder/mod.rs` (alongside the existing
`neon_mvni_pbt`).

### Test results

`cargo test --lib neon_two_misc` → **7 pass, 1 fail (by design)**:

- ✅ `prop_two_misc_matches_arm_layout` — encoded word == independent ARMv8 layout reference
- ✅ `prop_two_misc_fields_isolated` — Rd/Rn/opcode/size occupy exactly their fields
- ✅ `prop_two_misc_fixed_bits_and_qu_size` — constant fields invariant; Q/U/size driven by inputs
- ✅ `prop_two_misc_source_arrangement_ignored` — documents source arrangement discarded
- ✅ `golden_two_misc_matches_arm_layout`, `rejects_too_few_operands`, `rejects_unsupported_arrangement`
- ❌ `prop_two_misc_rejects_out_of_range_u_bit_and_opcode` — **BUG-1** (negative contract)

## Output Directories

| Path | Contents |
|------|----------|
| `src/backend/arm/assembler/encoder/` | added `neon_two_misc_pbt.rs`; one-line module registration in `mod.rs` |
| `pbt-out/bug_reports/` | this report (`encode-neon-two-misc-no-range-validation.md`) |
| `proptest-regressions/backend/arm/assembler/encoder/` | proptest auto-generated shrink seed for the failing property (`neon_two_misc_pbt.txt`) |
