# Bug Report: `encode_ldar_stlr` silently drops the memory offset

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldar_stlr`
**Severity:** High

## Summary

`encode_ldar_stlr` silently accepts and discards a non-zero immediate offset on
the memory operand, emitting an encoding that does not match the source text
instead of rejecting it. This is a silent mis-compilation: `ldar x0, [x1, #8]`
assembles to the same word as `ldar x0, [x1]`, loading from the *base* address
rather than the intended `x1 + 8`.

The ARMv8 ARM gives the acquire/release load/store group **no** immediate-offset
form. The only permitted addressing mode is `[Xn|SP]` — the effective address is
exactly the base register — and the encoding reflects this:

```
LDAR/STLR: size 001000 1 L 0 11111 1 11111 Rn Rt
 31 29..23 22 21 20..16 15 14..10 9..5 4..0
```

There is no immediate field at all, so a non-zero offset is unrepresentable and
must be rejected with `Err`. This is the same silent-drop defect class already
documented for the sibling exclusive/atomic encoders
(`encode_ldxr_stxr`, `encode_ldaxr_stlxr`, `encode_ldxp_stxp`,
`encode_cas`, `encode_swp`).

## Root Cause

```rust
pub(crate) fn encode_ldar_stlr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    let (rt, is_64) = get_reg(operands, 0)?;
    let rn = match operands.get(1) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
        _ => return Err("ldar/stlr needs memory operand".to_string()),
    };
    ...
}
```

The match arm binds `Operand::Mem { base, .. }` and discards `offset` via `..`.
The offset is never inspected, so any value — including an unrepresentable
non-zero offset — is silently accepted and the instruction is encoded as if the
offset were `0`.

## Reproduction

Input:

```
ldar x0, [x1, #8]
```

- **expected:** `Err` — `ldar x0, [x1, #8]` has no valid encoding (no offset
  field); GNU `as` rejects it with `Error: invalid addressing mode`.
- **actual:** `Ok(Word(0xC8DFFFE0))` — identical to `ldar x0, [x1]`. The offset
  `#8` is dropped; the load targets the base register.

Confirmed by the campaign's failing properties with minimal failing input
`is_load = false, rt_num = 0, base_num = 0, off = 1` → `Ok(Word(3365927936))`.

## Impact

- **Silent mis-compilation of atomics.** LDAR/STLR implement the ARMv8
  acquire/release memory model. Dropping the offset silently rewrites a
  programmer's intended address into the base register, producing a correctly
  *encoded* but semantically wrong acquire load / store release at the wrong
  address — exactly the kind of defect that survives review and fails only at
  runtime under concurrency.
- **Accepts unrepresentable input.** Any non-zero offset (positive, negative,
  aligned, page-spanning) is accepted instead of rejected, defeating the
  assembler's role as a spec gate.
- **Defect class consistency.** This is one of several encoders in the
  exclusive/atomic load/store family that drop `offset` via `{ base, .. }`;
  fixing it here should be done alongside the documented siblings.

## Suggested Fix

Bind and validate the offset:

```rust
let (rn, offset) = match operands.get(1) {
    Some(Operand::Mem { base, offset }) => {
        (parse_reg_num(base).ok_or("invalid base")?, *offset)
    }
    _ => return Err("ldar/stlr needs memory operand".to_string()),
};
if offset != 0 {
    return Err(format!(
        "ldar/stlr does not support an immediate offset (got #{offset}); \
         use a base-register addressing mode [Xn]"
    ));
}
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected`
(mod `prop_encode_ldar_stlr_offset_tests`, `load_store.rs`)

```rust
proptest! {
    #[test]
    fn prop_nonzero_offset_rejected(
        is_load in any::<bool>(),
        rt_num in 0u32..=31u32,
        base_num in 0u32..=31u32,
        off in (-32768i64..32767i64).prop_filter("non-zero", |o| *o != 0),
    ) {
        let ops = vec![Operand::Reg(format!("x{}", rt_num)),
                       Operand::Mem { base: format!("x{}", base_num), offset: off }];
        let res = encode_ldar_stlr(&ops, is_load, None);
        prop_assert!(res.is_err(),
            "non-zero offset {} on LDAR/STLR must be rejected (no offset field in encoding); got {:?}",
            off, res);
    }
}
```

Minimal failing input: `is_load = false, rt_num = 0, base_num = 0, off = 1`
→ `Ok(Word(3365927936))`. Expected: `Err`.

**GitHub Issue:** _none_
