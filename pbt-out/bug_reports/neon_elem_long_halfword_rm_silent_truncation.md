# BUG: `encode_neon_elem_long` silently truncates halfword (`.h`) Rm (V16–V31 alias V0–V15)

## Status
Confirmed / reproducible. Pinned by `halfword_rm_silently_aliases_v16_to_v0`
(passing, demonstrates the aliasing) and the `#[ignore]`d contract test
`rejects_halfword_rm_above_v15` (fails under the current implementation).

## Location
`src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_elem_long`

```rust
// Limit Rm for half-word indexing (only v0-v15)
let rm_enc = if size == 0b01 { rm & 0xF } else { rm & 0x1F };
```

## Symptom
For the halfword (`.h`) by-element long form (`SMULL`/`UMULL`/`SMLAL`/`UMLAL`/
`SMLSL`/`UMLSL`/`SQDMULL`/`SQDMLAL`/`SQDMLSL` by element), the register operand
`Vm` is constrained by the ARMv8-A ARM to **V0–V15** (the `M` bit at bit 20 is
sourced from the lane *index*, leaving only a 4-bit `Rm` field at bits 19–16).

`encode_neon_elem_long` never validates this constraint. Instead it masks with
`rm & 0xF`, so:

* `v16.h[0]` → `Ok(0x0F42A020)` — **identical** to `v0.h[0]`
* `v17.h[0]` → identical to `v1.h[0]`
* … and in general `v{N+16}.h[i]` aliases `v{N}.h[i]` for `N ∈ 0..15`.

An out-of-range `Vm` (V16–V31) is accepted and emits a silently-wrong word
pointing at the wrong vector register, with no `Err`.

### Reproduction
```
$ cargo test --lib rejects_halfword_rm_above_v15 -- --ignored
...
halfword by-element Rm must be V0-V15 (ARM DDI 0487); v16.h[0] must be rejected,
but got Ok(Word(255893536))   # 0x0F42A020 == encoding of v0.h[0]
```

The companion (non-ignored) test `halfword_rm_silently_aliases_v16_to_v0` pins
the exact aliasing: `v{N+16}.h[0] == v{N}.h[0]` for all `N ∈ 0..15`.

## Why this is wrong
* ARM DDI 0487 (ARMv8-A ARM), “Advanced SIMD vector by element”, long multiply
  group: the halfword form requires `Rm` in `0000000`–`0001111` (V0–V15) and the
  encoding is *unallocated* for `Rm >= 16`. (Contrast the word (`.s`) form,
  where `M = Rm[4]` so V0–V31 are legal — and the encoder handles that case
  correctly.)
* The code even documents the intent in a comment (`// only v0-v15`) yet performs
  a silent mask rather than a range check — exactly the asymmetry flagged: lane
  **indices** *are* validated (`if index > 7 { return Err }`), but the
  **register number** is not.

## Contrast with what already works
* Lane-index range **is** validated: `index > 7` for `.h` and `index > 3` for
  `.s` both return `Err`. ✓
* The word (`.s`) form reconstructs the full 5-bit `Rm` correctly via
  `M = Rm[4]` and the 4-bit `Rm` field. ✓
* All fixed fields, `Rd`, `Rn`, `U`, `opcode`, and the scattered
  `H(bit11):L(bit21):M(bit20)` index bits are correct for in-range inputs
  (verified against 8 hand-derived golden words and a differential reference
  encoder).

## Suggested fix
Before masking, reject an out-of-range `Rm` for the halfword form:

```rust
0b01 => {
    if rm > 15 {
        return Err(format!("element-long: halfword Vm must be v0-v15, got v{}", rm));
    }
    // … existing index handling …
}
```

(and keep `rm_enc = rm & 0xF`, now provably a no-op for valid input).

## Impact
Silent mis-assembly: any source using `V16–V31` as the by-element operand of a
halfword long multiply produces an object that references the wrong register,
with no diagnostic. Low frequency (depends on register allocation hitting the
upper half of the V register file for this instruction form) but high severity
when it occurs (wrong code, no warning).
