# Bug Report — `encode_umull` silently ignores register widths

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_umull`
**Severity:** Medium (wrong code generation / spec violation, no crash)
**Status:** Confirmed by property-based test (1 failing, 3 documenting).

## Summary

`encode_umull` decodes each operand's *number* but discards its *width* (the
`is_64` half of `get_reg`'s return value). It therefore accepts
`umull w0, w1, w2` — which has **no valid AArch64 encoding** — and emits the
word for `umull x0, w1, w2` instead. The destination width, source widths, and
the fixed `sf=1` are never cross-checked.

## Spec basis (ARMv8 ARM)

`UMULL <Xd>, <Wn>, <Wm>` is the alias of `UMADDL <Xd>, <Wn>, <Wm>, <XZR>` whose
encoding fixes `sf = 1` (64-bit destination). The destination **must** be a
64-bit (X) register and the sources **must** be 32-bit (W) registers. A 32-bit
destination is UNPREDICTABLE / unallocated. Reference assemblers (GAS, LLVM-MC)
reject `umull w0, w1, w2` with an "operand mismatch" error.

## Reproduction

```rust
// umull w0, w1, w2  — SHOULD be Err, currently returns Ok
encode_umull(&[Operand::Reg("w0".into()),
               Operand::Reg("w1".into()),
               Operand::Reg("w2".into())])
// => Ok(EncodeResult::Word(0x9BA07C02))   // identical to umull x0, w1, w2
```

Minimal failing input surfaced by property `umull_destination_must_be_64bit`:
`n = 0` → `umull w0, w0, w0`.

## Root cause

```rust
pub(crate) fn encode_umull(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    ...
}
```

`get_reg` returns `(num, is_64)`; all three bindings use `_`, so:
- the destination width is never validated (must be X),
- the source widths are never validated (must be W),
- `sf` is hardcoded to `1` regardless.

## Properties added (in-module `proptest!` block)

| Property | Result | Role |
|---|---|---|
| `umull_destination_must_be_64bit` | **FAILS** | Spec negative contract: W destination → must be `Err` |
| `umull_dest_width_silently_ignored` | passes | Documents dead `is_64`: `w{n}` and `x{n}` destinations encode identically |
| `umull_source_width_silently_ignored` | passes | Documents dead `is_64`: X/W sources encode identically |
| `umull_sf_set_even_for_all_w_operands` | passes | `sf==1` even when every operand is W (proves width never inspected) |

## Suggested fix

Validate widths against the `UMULL <Xd>, <Wn>, <Wm>` contract before encoding:

```rust
pub(crate) fn encode_umull(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, rd_is64) = get_reg(operands, 0)?;
    let (rn, rn_is64) = get_reg(operands, 1)?;
    let (rm, rm_is64) = get_reg(operands, 2)?;
    if !rd_is64 { return Err("umull requires a 64-bit destination (Xd)".into()); }
    if rn_is64  { return Err("umull requires a 32-bit source (Wn)".into()); }
    if rm_is64  { return Err("umull requires a 32-bit source (Wm)".into()); }
    let word = (1u32 << 31) | (0b0011011101 << 21) | (rm << 16)
        | (0b011111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Scope note

The identical bug affects every "long multiply" sibling that binds `is_64` to
`_`: `encode_smull`, `encode_smaddl`, `encode_umaddl`, `encode_smulh`,
`encode_umulh`, `encode_mneg` (and the pre-existing
`smull_rejects_wrong_width_destination` test, which also fails for the same
reason). The encoding bits themselves are correct; only the width validation is
missing.
