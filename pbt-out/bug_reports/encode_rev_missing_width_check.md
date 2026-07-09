# Bug: `encode_rev` silently accepts mismatched Rd/Rn register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` — `encode_rev`
**Severity:** Correctness (emits architecturally UNPREDICTABLE encoding)
**Status:** Confirmed by property test (expected failure)

## Summary

`encode_rev` derives the `sf` (size) bit solely from the destination register
`Rd` and **ignores the width of the source register `Rn`**. As a result, a
mismatched-width pair such as `REV w0, x0` or `REV x0, w1` is silently
accepted and encoded as if both operands shared `Rd`'s width.

## Root cause

```rust
pub(crate) fn encode_rev(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // <-- width discarded
    let sf = sf_bit(is_64);
    let opc = if is_64 { 0b000011 } else { 0b000010 };
    let word = ((sf << 31) | (1 << 30) | (0b011010110 << 21))
        | (opc << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`is_64` comes only from `operands[0]`. `Rn`'s width is bound to `_` and never
validated against `Rd`'s width.

## Why it is a bug

The AArch64 REV encoding (ARM ARM, *Data-processing (1 source)*) carries a
single `sf` bit:

```
sf 1 0 11010110 00000 opc[15:10] Rn Rd
```

Because there is only one size field, `Rd` and `Rn` **must share the same
width** (both W or both X). A `REV Xd, Wn` / `REV Wd, Xn` pair is
UNPREDICTABLE / UNALLOCATED and a correct assembler must reject it.

## Reproduction

`REV w0, x0` (source wider than destination) currently succeeds and emits the
**32-bit** REV word:

```
encode_rev(&[Reg("w0"), Reg("x0")])  ==  Ok(Word(0x5AC0_0800))
```

`sf` is 0 (taken from `w0`) and `Rn=0` is written into the source field even
though the operand was `x0` — the source register's width information is lost
and the emitted instruction no longer matches its operands.

## Property test

`prop_encode_rev_tests::prop_rejects_mismatched_widths` (expected failure):

```
test prop_rejects_mismatched_widths ... FAILED
minimal failing input: rd = 0, rn = 0, rd_is_64 = false
REV with mismatched Rd/Rn widths (w0,x0) should be rejected,
got Ok(Word(1522534400))   // 0x5AC0_0800
```

The other five properties (field placement, full ARM-ARM reference match for
both widths, width differential, determinism, malformed-operand rejection)
all **pass**, confirming the bit layout itself is correct — the only defect
is the missing width-consistency check.

## Suggested fix

In `encode_rev`, validate that `Rn`'s width matches `Rd`'s:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;
if is_64 != rn_is_64 {
    return Err(format!(
        "REV requires Rd and Rn of the same width, got {:?}, {:?}",
        operands[0], operands[1]
    ));
}
```

## Note on scope

The same `let (rn, _) = get_reg(...)` width-discarding pattern is present in
every bit-manipulation encoder in this file (`encode_rev`, `encode_rev16`,
`encode_rbit`, `encode_clz`, `encode_cls`, etc.), so the fix should likely be
applied consistently — but `encode_rev` is the immediate confirmed instance.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/154
