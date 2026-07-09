# Bug Report: `encode_swp` silently drops memory offset (no range validation)

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_swp`
**Severity:** High

## Summary

`encode_swp` accepts any memory operand `Operand::Mem { base, offset }` and silently
discards the `offset`. The SWP family (SWP/SWPA/SWPAL/SWPL and the byte/halfword variants
SWPB/SWPH/..., ARMv8.1-A LSE atomics, ARM ARM §C6.2.272 SWP) has **no immediate-offset
field in its encoding** — the only permitted addressing form is `[Xn|SP]`. Consequently a
non-zero offset is *unrepresentable* and must be rejected with `Err`, but the encoder
happily emits `[Xn]` for e.g. `swp x0, x1, [x2, #8]` or even a negative offset
`swp x0, x1, [x2, #-1]`, producing a word identical to offset 0.

This is the same silent-drop defect already documented for the sibling functions
`encode_ldxr_stxr` and `encode_ldxp_stxp` (see the existing `*_offset_tests` modules).
A programmer who writes `swp x1, x0, [x4, #16]` intending an offset access gets, with no
diagnostic, a swap against `[x4]` — a different address, corrupting an unintended memory
location.

## Root Cause

The memory-operand matcher uses a wildcard field and never inspects `offset`:

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("swp: invalid base")?,
    _ => return Err("swp requires memory operand [Xn]".to_string()),
};
```

The SWP encoding built afterwards contains no offset bits:

```rust
// size 111000 A R 1 Rs 1 000 00 Rn Rt
let word = (size << 30) | (0b111000 << 24) | (a << 23) | (r << 22) | (1 << 21)
    | (rs << 16) | (1 << 15) | (rn << 5) | rt;
```

Because the ARM ARM form is `SWP <Xs>, <Xt>, [<Xn|SP>]` (no offset, no pre/post-index),
`offset` has nowhere to go and is dropped.

## Reproduction

```
swp x0, x1, [x0, #-1]   -> Ok(Word(4162879488))   // == 0xF8208040, identical to offset 0
swp x0, x1, [x2, #-2]   -> Ok(Word(4162879553))   // == 0xF8208041, the [x2] encoding
swp x0, x1, [x2, #8]    -> Ok(Word(4162879553))   // same word; #8 silently lost
```

Expected: `Err` for every non-zero offset. Actual: `Ok` with the offset dropped.

Confirmed by proptest minimal failing inputs:
- `mn = "swp", rs_num = 0, rt_num = 0, base_num = 0, off = -1`
- `mn = "swp", off = -2`

## Impact

Silent mis-assembly. An atomic swap (a concurrency primitive) is redirected to the wrong
address with no diagnostic. Because SWP is used for lock-free data structures, an offset
that the programmer believes is in effect but is actually dropped can corrupt unrelated
memory and cause data races that are extremely hard to diagnose. The negative-offset case
is especially dangerous: there is no plausible "wrapping" interpretation, yet it is
accepted without error.

## Suggested Fix

Bind and validate the offset explicitly, rejecting any non-zero value (only `[Xn|SP]` is
legal). While here, apply the same fix to the sibling `encode_cas` matcher directly above
`encode_swp`, which has the identical `Operand::Mem { base, .. }` pattern and the same
unrepresentable-offset class.

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, offset }) => {
        if *offset != 0 {
            return Err(format!(
                "{}: only [<Xn|SP>] is permitted; immediate offset {} is not encodable",
                mnemonic, offset
            ));
        }
        parse_reg_num(base).ok_or("swp: invalid base")?
    }
    _ => return Err("swp requires memory operand [Xn]".to_string()),
};
```

## Regression Property

Failing properties: `prop_nonzero_offset_rejected`, `prop_common_offsets_rejected`,
`prop_negative_offset_never_encoded` (module `prop_encode_swp_offset_tests`). The
complementary `prop_offset_does_not_affect_word` passes today and pins the bug mechanism
(two distinct offsets yield the same word).

```rust
#[test]
fn prop_nonzero_offset_rejected(
    mn in arb_mn(),
    rs_num in 0u32..=31u32,
    rt_num in 0u32..=31u32,
    base_num in 0u32..=31u32,
    off in (-32768i64..32767i64).prop_filter("non-zero", |o| *o != 0),
) {
    let res = encode_swp(mn, &swp_ops('x', rs_num, 'x', rt_num, base_num, off));
    prop_assert!(res.is_err(),
        "non-zero offset {} on {} must be rejected (no offset field in encoding); got {:?}",
        off, mn, res);
}
```

Minimal failing case: `encode_swp("swp", &[Reg("x0"), Reg("x1"), Mem { base: "x0", offset: -1 }])`
returns `Ok(Word(0xF8208040))`; expected `Err`.

**GitHub Issue:** <link if created>
