# Bug — `encode_ret`: 32-bit W-form register wrongly accepted

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_ret`

```rust
pub(crate) fn encode_ret(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rn = if operands.is_empty() {
        30 // default to x30 (LR)
    } else {
        get_reg(operands, 0)?.0          // <-- discards the is_64 flag
    };
    let word = 0xd65f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

`get_reg` returns `(num, is_64)`, but `encode_ret` binds only `.0` and throws away
the width. The `RET` instruction's operand is a 64-bit **general-purpose** register
`<Xn>` (ARM ARM C5.6.20); the 32-bit `W` form is **unallocated** and a conforming
assembler rejects it:

```
$ echo "ret w0" | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
```

Because the width is ignored, `ret w0` and `ret x0` emit byte-identical words.

## Minimal input

| Mnemonic | Encoded word | Reference (clang) | Expected here |
|---|---|---|---|
| `ret w0` | `0xD65F0000` (== `ret x0`) | error: invalid operand | `Err` |
| `ret w30` | `0xD65F0000 \| (30<<5)` (== `ret x30`) | error | `Err` |

## Actual behavior (observed failure)

`encode_ret(&[Operand::Reg("w0".into())])` returns
`Ok(EncodeResult::Word(3596550144))` — `3596550144 == 0xD65F0000`, bit-identical to
`encode_ret(&[Operand::Reg("x0".into())])`. Confirmed by a **failing** proptest:

```
prop_rejects_32bit_w_form
  panicked: ret w0 must be rejected (32-bit form is unallocated per ARM ARM C5.6.20),
            got Ok(Word(3596550144))
  minimal failing input: n = 0
```

## Impact

Silent mis-assembly of an architecturally unallocated instruction with no
diagnostic. The same width-discard defect affects the sibling `encode_br` and
`encode_blr` (documented separately) and any other encoder that binds
`(rn, _) = get_reg(...).0`.

## Property that locks it (FAILING — bug confirmed)

`prop_encode_ret_tests::prop_rejects_32bit_w_form` (in `compare_branch.rs`) is a
**negative-contract** property asserting `encode_ret(&[Reg("wN")]).is_err()` for
`N in 0..=30`. It **FAILS** against the current implementation. Once validation is
added it passes unchanged.

## Fix

Reject the `W` form by inspecting `is_64`:

```rust
let rn = if operands.is_empty() {
    30
} else {
    let (num, is_64) = get_reg(operands, 0)?;
    if !is_64 {
        return Err("ret requires a 64-bit register (Xn)".into());
    }
    num
};
```
