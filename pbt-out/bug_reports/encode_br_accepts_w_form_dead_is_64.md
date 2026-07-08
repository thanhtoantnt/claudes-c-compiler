# Bug — `encode_br` accepts the 32-bit `W` register form (dead `is_64`)

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_br`
**Severity:** Correctness / assembler-conformance (silent mis-assembly of unallocated encoding)
**Pinned by:** `prop_encode_br_tests::prop_width_independent_w_form_accepted`

## Summary

`encode_br` binds `(rn, _) = get_reg(operands, 0)?`, **discarding** the `is_64`
flag that `get_reg` already computed. Consequently the 32-bit `W` form `br w0`
is accepted and encodes **bit-identically** to `br x0`, even though per the ARM
ARM (C5.6.17) `BR`'s sole operand is `<Xn>` and the 32-bit `W` form is an
**unallocated encoding**.

```rust
pub(crate) fn encode_br(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;                 // is_64 DISCARDED
    let word = 0xd61f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## Minimal input

```
br w0
```

## Expected vs actual

| | value |
|---|---|
| **Expected** (GAS / `llvm-mc` behavior) | `Err` — `br w0` is unallocated; `BR` requires a 64-bit `X` register |
| **Actual** | `Ok(Word(0xd61f0000))` — identical to `br x0`, accepted silently |

A conforming assembler rejects it:
```
Error: operand 1 must be a 64-bit register -- `br w0'
```

## Impact

The assembler silently emits a valid-looking `BR X0` for input the programmer
almost certainly did not intend (a 32-bit pointer truncation). This is a
silent correctness loss: mis-assembled branch target with no diagnostic.

## Root cause

`get_reg` returns the register width (`is_64`), but `encode_br` discards it
with `(_, _)`. `BR` is inherently a 64-bit instruction; its width must be
enforced, not ignored.

## Suggested fix

```rust
let (rn, is_64) = get_reg(operands, 0)?;
if !is_64 {
    return Err("br requires a 64-bit X register".into());
}
let word = 0xd61f0000 | (rn << 5);
Ok(EncodeResult::Word(word))
```

When landed, `prop_width_independent_w_form_accepted`'s `is_ok()` assertion
should flip to `is_err()`.

## Reproduce
```bash
cargo test --lib prop_width_independent_w_form_accepted
```
