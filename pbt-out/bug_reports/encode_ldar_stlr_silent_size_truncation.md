# Bug Report — `encode_ldar_stlr` silently corrupts out-of-range `forced_size`

## Location
`src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldar_stlr`
(signature: `fn encode_ldar_stlr(operands, is_load, forced_size: Option<u32>) -> Result<EncodeResult, String>`).

## Summary
The `size` field is computed as `forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 })`
and placed into bits `[31:30]` via `size << 30` **without any range validation**.
The ARMv8-A Architecture Reference Manual only allocates `size` encodings `0b00`–`0b11`
for the LDAR/STLR instruction family (LDAR/STLR, LDARB/STLRB, LDARH/STLRH; size=10/11
are the 32/64-bit forms). Any `forced_size` value `>= 4` therefore has no valid
encoding and *should* be rejected with `Err`, but instead it silently shifts off the
high bits and emits a corrupt instruction word.

## Root cause
```rust
let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
...
let word = ((size << 30) | (0b001000 << 24) | (1 << 23) | (l << 22))
    | (0b11111 << 16) | (1 << 15) | (0b11111 << 10) | (rn << 5) | rt;
```
`size << 30` for `size >= 4` discards the bits above position 31. Examples:
- `forced_size = Some(4)`  → `4u32 << 30 == 0` → `size` field becomes `00` (LDARB) silently.
- `forced_size = Some(5)`  → `5u32 << 30 == 0x40000000` → `size` field becomes `01` (LDARH) silently.
- `forced_size = Some(255)`→ `size` field becomes `11` silently.

No `Err` is ever produced for these unallocatable values.

## Reproduction (property test)
The committed property `prop_forced_size_out_of_range_rejected` (marked `#[ignore]`)
asserts the correct contract — that `forced_size` in `4..=255` returns `Err`. Running it:

```
$ cargo test --lib prop_forced_size_out_of_range_rejected -- --ignored
...
panicked: Test failed: forced_size=4 (>3) must be rejected,
           got Ok(Ok(Word(144702496)))
minimal failing input: bad_size = 4, is_load = false
```
`144702496 == 0x089FFC00` is the LDAR/STLR constant skeleton with `size=00` — i.e.
a `forced_size` of 4 was silently re-encoded as a byte-sized atomic access.

## Impact
- **Severity: Low.** `encode_ldar_stlr` is `pub(crate)` and all six internal
  callers (`mod.rs` mnemonic dispatch for `ldar`/`stlr`/`ldarb`/`stlrb`/`ldarh`/`stlrh`)
  only ever pass `None`, `Some(0b00)`, or `Some(0b01)`, so the bug is not reachable
  through the public assembler today.
- **Robustness / latent risk:** any future caller, fuzz input, or hand-written test
  that passes an out-of-range `forced_size` will get a plausible-looking but
  architecturally *UNDEFINED* encoding with no diagnostic. Because the function
  already returns `Result<_, String>` and validates other inputs (e.g. the memory
  operand shape), the missing size-range check is an inconsistency in the error
  contract rather than a deliberate design choice.

## Suggested fix
Reject unallocatable size values before encoding, e.g.:

```rust
let size = match forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 }) {
    s @ 0..=3 => s,
    other => return Err(format!("ldar/stlr: size field out of range (0..=3): {}", other)),
};
```

## Verified correct behavior (for reference)
The following hand-derived golden encodings (ARM ARM §C4 LDAR/STLR layout) all pass:

| Instruction    | Encoding     |
|----------------|--------------|
| LDAR  X0,[X1]  | `0xC8DFFC20` |
| STLR  X0,[X1]  | `0xC89FFC20` |
| LDAR  W0,[X1]  | `0x88DFFC20` |
| STLR  W0,[X1]  | `0x889FFC20` |
| LDARB W0,[X1]  | `0x08DFFC20` |
| LDARH W0,[X1]  | `0x48DFFC20` |

Field placement (Rt[4:0], Rn[9:5]), the load/store `L`-bit differential
(`LDAR ^ STLR == 0x00400000`), and the constant skeleton invariant
(`word & 0x3FBFFC00 == 0x089FFC00`) all hold for every in-range input.

## Regression property

Failing property: `prop_forced_size_out_of_range_rejected`

```rust
prop_assert!(encode_ldar_stlr(&[xreg(0), mem(xreg(1))], false, 4).is_err());
```
