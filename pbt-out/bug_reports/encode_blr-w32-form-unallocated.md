# Bug — `encode_blr`: 32-bit `W` form wrongly accepted (unallocated encoding)

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_blr`

```rust
pub(crate) fn encode_blr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;   // <-- is_64 discarded
    let word = 0xd63f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

`get_reg` returns `(num, is_64)` but `encode_blr` binds `(rn, _)` and **discards** the `is_64` flag. As a result `blr w0` encodes bit-identically to `blr x0`. Per the ARM ARM (C5.6.18), `BLR`'s sole operand is `<Xn>` — a **64-bit** general-purpose register. The 32-bit `W` form is **unallocated**; a conforming assembler (GAS, `llvm-mc`) rejects it:

```
$ echo "blr w0" | llvm-mc -triple=aarch64 -show-encoding
error: invalid operand for instruction
```

## Minimal input

| Mnemonic | Encoded word | Expected |
|---|---|---|
| `blr w0` | `0xD63F0000` (== `blr x0`) | `Err` (unallocated encoding) |
| `blr w30` | `0xD63F0000 | (30<<5)` (== `blr x30`) | `Err` |

## Actual behavior (observed failure)

`encode_blr(&[Operand::Reg("w0".into())])` returns `Ok(EncodeResult::Word(0xD63F0000))`, bit-identical to `encode_blr(&[Operand::Reg("x0".into())])`. No diagnostic; the `W` mnemonic is silently rewritten to `X`. Confirmed by a **failing** proptest run:

```
prop_rejects_32bit_w_form
  panicked: blr w0 must be rejected (...), got Ok(Word(3594452992))
  minimal failing input: n = 0      (3594452992 == 0xD63F0000)
```

## Impact

Silent mis-assembly. A program that (mistakenly or via a parser quirk) emits `blr wN` gets valid-looking machine code that branches to the address held in the **full 64-bit** `xN` register — semantically wrong relative to the source, with no assembler error to catch the mistake. The 32-bit `W` encoding was never allocated by the architecture, so any toolchain consumer relying on rejection-as-validation is misled.

## Property that locks it (FAILING — bug confirmed)

`prop_encode_blr_tests::prop_rejects_32bit_w_form` (in `compare_branch.rs`) is a **negative-contract** property asserting `encode_blr(wN).is_err()`. It **FAILS** against the current implementation (`blr w0 → Ok(0xD63F0000)`). Once validation is added the property will pass; no assertion changes are needed.

## Fix

```rust
let (rn, is_64) = get_reg(operands, 0)?;
if !is_64 {
    return Err(format!("blr: operand must be 64-bit (Xn)"));
}
```
