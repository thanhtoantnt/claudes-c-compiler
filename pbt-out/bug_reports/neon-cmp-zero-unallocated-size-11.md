# `encode_neon_cmp_zero` — unallocated `size=11` arrangements silently accepted

**Target:** `src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_cmp_zero`
**Severity:** Medium (user-facing: an unallocated instruction is emitted without any error)
**Status:** Open. Demonstrated by the `#[ignore]`d test
`rejects_unallocated_size_11_arrangements` in `neon_cmp_zero_pbt.rs`.

## Summary

`encode_neon_cmp_zero` correctly packs every fixed field and register field of
the "Advanced SIMD two-register miscellaneous" compare-to-zero encoding
(verified by the golden table and the differential/field-decomposition
properties, which all PASS). However it does **not** reject arrangements that
map to `size = 0b11` (`1d`, `2d`), which are architecturally **UNALLOCATED**
for the compare-to-zero instruction group. A user can therefore write e.g.

```
cmeq v0.2d, v1.2d, #0
```

and the assembler silently emits an unallocated 32-bit word instead of
returning `Err`.

## Root cause

`neon_arr_to_q_size` maps both `.1d` and `.2d` to `size = 0b11`, and
`encode_neon_cmp_zero` uses that value unconditionally:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;          // 1d/2d -> size=11
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22)
    | (0b10000 << 17) | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
```

No check rules out `size == 0b11` for this instruction group.

## Architectural basis

Per the ARMv8-A ARM (ARM DDI 0487), "Advanced SIMD two-register miscellaneous",
the compare-with-zero instructions `CMEQ`/`CMGE`/`CMGT`/`CMLE`/`CMLT Vd.T, Vn.T,
#0` are defined **only** for `T ∈ {8B, 16B, 4H, 8H, 2S, 4S}` — i.e. `size ∈
{00, 01, 10}`. The encoding field `size = 11` is UNALLOCATED for this group,
so any emitted word with `size=11` is UNDEFINED behaviour on the target.

(Contrast: the register-form `CMEQ Vd.T, Vn.T, Vm.T` is a *different* encoding
group and is out of scope here. This report concerns only the `#0` form that
`encode_neon_cmp_zero` serves.)

## Reproduction

```
cargo test --lib -- --ignored neon_cmp_zero_pbt::rejects_unallocated_size_11_arrangements
```

```
compare-to-zero does not support .1d; expected Err but got Ok(0x0EE09820)
compare-to-zero does not support .2d; expected Err but got Ok(0x4EE09820)
```

`0x0EE09820` / `0x4EE09820` both carry `(word >> 22) & 0x3 == 0b11` — i.e. the
UNALLOCATED `size=11` encoding.

## Suggested fix

Reject `size == 0b11` (equivalently arrangements `1d`/`2d`) before encoding,
mirroring the validation done for the destination arrangement elsewhere:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 {
    return Err(format!(
        "compare-to-zero does not support arrangement {} (size=11 is UNALLOCATED)",
        arr_d));
}
```

## Secondary observation (low severity — not a correctness bug)

The `u_bit` and `opcode` parameters are neither masked nor range-checked:
`u_bit << 29` and `opcode << 12` are applied verbatim. An out-of-range `u_bit`
(e.g. `2`) would silently set the adjacent `Q` bit (bit 30), and an out-of-range
`opcode` (e.g. `0x20`) would clobber the fixed `10000` field at bits 21-17.
This is not currently reachable in practice because the only callers are the
hardcoded dispatcher entries in `mod.rs` (which pass well-formed constants), so
it is recorded only as a hardening note, not as a failing test.
