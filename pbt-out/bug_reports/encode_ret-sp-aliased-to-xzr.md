# Bug — `encode_ret`: `sp`/`wsp` silently aliased to `xzr`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_ret`

```rust
pub(crate) fn encode_ret(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rn = if operands.is_empty() {
        30 // default to x30 (LR)
    } else {
        get_reg(operands, 0)?.0
    };
    let word = 0xd65f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

The shared helper `parse_reg_num` (in `encoder/mod.rs`) maps **both** `sp` and
`xzr` to the encoding value `31`:

```rust
"sp" | "wsp" => Some(31),
"xzr" | "wzr" => Some(31),
```

`RET`'s `Rn` field value `31` denotes **XZR**; there is **no** SP-using form of
`RET`. So `ret sp` currently succeeds and silently encodes a return to the address
held in XZR (== `ret xzr`) instead of erroring; `ret wsp` behaves identically. A
conforming assembler rejects both:

```
$ echo "ret sp"  | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
$ echo "ret wsp" | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
```

## Minimal input

| Mnemonic | Encoded word | Reference (clang) | Expected here |
|---|---|---|---|
| `ret sp`  | `0xD65F03E0` (== `ret xzr`, Rn=31) | error: invalid operand | `Err` |
| `ret wsp` | `0xD65F03E0` (== `ret xzr`, Rn=31) | error | `Err` |

## Actual behavior (observed failure)

`encode_ret(&[Operand::Reg("sp".into())])` returns
`Ok(EncodeResult::Word(3596551136))` — `3596551136 == 0xD65F03E0`, i.e. Rn=31,
bit-identical to `encode_ret(&[Operand::Reg("xzr".into())])`. Confirmed by a
**failing** proptest:

```
prop_rejects_sp_wsp
  panicked: ret sp must be rejected (SP/WSP is not a valid RET operand;
            field 31 == XZR), got Ok(Word(3596551136))
  minimal failing input: which = 0
```

## Impact

Silent mis-assembly: a user writing `ret sp` gets a return through XZR (address 0)
with no diagnostic — a severe codegen bug. The identical SP→XZR aliasing defect
affects the sibling `encode_br` and `encode_blr` (documented separately).

## Property that locks it (FAILING — bug confirmed)

`prop_encode_ret_tests::prop_rejects_sp_wsp` (in `compare_branch.rs`) is a
**negative-contract** property asserting `encode_ret(&[Reg("sp")])` and
`encode_ret(&[Reg("wsp")])` return `Err`. It **FAILS** against the current
implementation. Once validation is added it passes unchanged.

## Fix

Reject the SP/WSP spellings explicitly (they are not valid `RET` operands):

```rust
if let Some(Operand::Reg(name)) = operands.get(0) {
    let lo = name.to_lowercase();
    if lo == "sp" || lo == "wsp" {
        return Err("ret does not accept SP/WSP (Rn field 31 denotes XZR)".into());
    }
}
```
