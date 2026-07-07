# Bug: `encode_ldrsw` silently truncates out-of-range memory offsets

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `encode_ldrsw` — `Operand::Mem { base, offset }` arm
**Severity:** High (silent mis-compilation: wrong machine code emitted with no diagnostic)
**Found by:** property `prop_encode_ldrsw_tests::prop_out_of_range_mem_offset_rejected` (fails)

## Summary

The `[base, #offset]` (unsigned-offset) form of `encode_ldrsw` first tries the
unsigned-offset encoding, and on failure falls through to the **unscaled**
(LDURSW) encoding. The unscaled branch unconditionally masks the offset to 9 bits
via `(*offset as i32) & 0x1FF` **without checking that the offset fits in the
9-bit signed `imm9` field (range [-256, 255])**.

Any offset that is out of the encodable union is therefore silently wrapped
into the 9-bit field and emitted as a *different, valid-looking* instruction.

## Affected code

```rust
Some(Operand::Mem { base, offset }) => {
    let rn = parse_reg_num(base).ok_or("invalid base reg")?;
    // ... unsigned-offset attempt (imm12 < 4096, %4 == 0) ...
    // Unscaled: LDURSW
    let imm9 = (*offset as i32) & 0x1FF;                       // ← NO RANGE CHECK
    let word = (((0b10 << 30) | (0b111 << 27)) | (0b10 << 22)
        | ((imm9 as u32 & 0x1FF) << 12)) | (rn << 5) | rt;
    return Ok(EncodeResult::Word(word));
}
```

## Encodable range (ARMv8-A ARM, §C4.1.65)

The `[base, #imm]` operand form is encodable by **exactly one** of:
- **LDRSW (unsigned offset):** `imm12 * 4`, `imm12 ∈ [0, 4095]` → `imm ∈ {0, 4, …, 16380}`, 4-aligned.
- **LDURSW (unscaled):** signed `imm9 ∈ [-256, 255]`.

So the encodable union is `[-256, 255] ∪ {multiples of 4 in [0, 16380]}`.
Anything outside this union is **not representable** and the ARM ARM mandates
the assembler reject it (GNU `as` errors: `Error: immediate out of range`).

## Reproduction

```
off = 16384   →  encodes as Word(0xB8800000)   == ldursw x0, [x1, #0]
```

- `16384 % 4 == 0` but `16384 / 4 = 4096`, which is **not** `< 4096`, so the
  unsigned-offset form is correctly skipped.
- The unscaled branch then computes `16384 & 0x1FF = 0` and happily emits
  `ldursw x0, [x1, #0]` — i.e. `ldrsw x0, [x1, #16384]` becomes a load from
  `[x1, #0]`, silently.

Other inputs with the same defect:
- `off = 20000` → `imm9 = 20000 & 0x1FF = 32` → encodes as `ldursw … [x1, #32]`.
- `off = -300`  → `imm9 = -300 & 0x1FF = 212` → sign-extends to `-300`? No —
  `212 & 0x1FF = 212`, bit 8 set → `-100`. Wrong offset.
- `off = 301`   → not a multiple of 4 and `> 255` → `imm9 = 301` → sign-extends to `-211`.

## Impact

Wrong load/store addresses are emitted into object code with no assembler error.
This produces silently incorrect binaries (loads from the wrong address), which
is far worse than a build failure. The sibling functions `encode_ldr_str`,
`encode_ldrs`, and `encode_ldp_stp` use the same mask-without-range-check
pattern in their unscaled fall-through branches and are likely affected too
(`encode_ldur_stur` itself also masks `imm9` without rejecting `|imm9| > 255`).

## Suggested fix

In the unscaled fall-through, validate the offset before masking, e.g.:

```rust
// Unscaled: LDURSW — imm9 is a signed 9-bit field.
let imm9_val = *offset as i32;
if !(-256..=255).contains(&imm9_val) {
    return Err(format!(
        "ldrsw: offset {} out of range; must be a multiple of 4 in [0, 16380] (unsigned) or in [-256, 255] (unscaled)",
        offset
    ));
}
let imm9 = (imm9_val as u32) & 0x1FF;
```

## Properties written

In `load_store.rs`, module `prop_encode_ldrsw_tests` (5 properties, proptest):

1. `prop_unsigned_offset_layout` — field layout vs golden `0xB9800020` (**pass**).
2. `prop_rt_rn_field_placement_all_forms` — Rt[4:0], Rn[9:5] across all 4 forms (**pass**).
3. `prop_pre_post_index_imm9_and_marker` — imm9 sign-extends, markers 11/01, goldens (**pass**).
4. `prop_reg_offset_fields` — Rm[20:16], option[15:13], S[12] (**pass**).
5. `prop_out_of_range_mem_offset_rejected` — negative contract (**FAIL** → this bug).
