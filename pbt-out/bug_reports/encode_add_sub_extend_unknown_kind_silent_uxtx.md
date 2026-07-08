# Bug — `encode_add_sub` silently rewrites unknown extend mnemonic as UXTX

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` →
`encode_add_sub`, the `Operand::Extend { .. }` branch.

## Summary

The match that maps an extend mnemonic to the `option` field has a
catch-all arm `_ => 0b011` (UXTX). Any unrecognized `kind` — typos,
fabricated mnemonics, anything not in the eight valid kinds — is silently
encoded as a valid `... , uxtx` instruction instead of returning an error.

## Relevant code

```rust
if let Some(Operand::Extend { kind, amount }) = operands.get(3) {
    let option = match kind.as_str() {
        "uxtb" => 0b000u32,
        "uxth" => 0b001,
        "uxtw" => 0b010,
        "uxtx" => 0b011,
        "sxtb" => 0b100,
        "sxth" => 0b101,
        "sxtw" => 0b110,
        "sxtx" => 0b111,
        _ => 0b011, // default UXTX/LSL        // <-- FINDING
    };
    ...
}
```

## Differential check (clang `--target=aarch64`)

```
$ echo 'add x0, x1, x2, foo' | clang --target=aarch64-linux-gnu -c -o /dev/null -
error: expected 'sxtx' 'uxtx' or 'lsl' with optional integer in range [0, 4]
```

`foo` is rejected by LLVM; the encoder accepts it and emits the bits for
`uxtx`.

## Property test (EXPECTED FAIL)

`add_extended_register_rejects_unknown_extend_kind` — for any `kind` not in
{uxtb,uxth,uxtw,uxtx,sxtb,sxth,sxtw,sxtx}, `encode_add_sub(...).is_err()`.
Minimal failing input: `kind = "uxzz"`.

## Fix

Replace the `_ => 0b011` arm with an explicit `Err`. The only legitimate
alias to consider is `lsl` (accepted as a synonym for UXTX on 64-bit
operands / UXTW on 32-bit); handle it explicitly rather than via a
catch-all that masks typos.

## Reproduce

```
cargo test --lib data_processing::tests::add_extended_register_rejects_unknown_extend_kind
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/125
