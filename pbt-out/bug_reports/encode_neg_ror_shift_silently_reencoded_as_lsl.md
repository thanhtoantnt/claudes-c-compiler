# Bug — `encode_neg` silently re-encodes `ror` shift as LSL

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_neg` (NEG = alias of `SUB Rd, XZR, Rm [, shift]`)
**Found by:** property-based test `data_processing::tests::neg_rejects_ror_shift`

## Defect

`encode_neg` accepts a `ror` shift operand and silently encodes it as `LSL`
instead of rejecting it.

**Spec:** ARMv8 ARM §C4.1.66 — the add/sub shifted-register form only permits
**LSL / LSR / ASR**. `ROR` is reserved for the logical shifted-register class
(AND/ORR/EOR/...). Supplying `ror` to an add/sub-family mnemonic is UNDEFINED.

## Code

```rust
let st = match kind.as_str() {
    "lsl" => 0b00u32,
    "lsr" => 0b01,
    "asr" => 0b10,
    _ => 0b00,                 // ← "ror" (and any unknown kind) silently -> LSL
};
```

## Reproduction

`cargo test --lib data_processing::tests::neg_rejects_ror_shift`

```
minimal failing input: rd = 0, rm = 0, amount = 0, is_64 = false   (neg w0, w0, ror #0)
assertion failed: encode_neg(&ops).is_err()
```

`neg w0, w0, ror #0` returns `Ok` with a word that encodes `lsl #0`. Expected:
`Err`, as GAS/LLVM do. (The sibling `encode_negs` has the same fallback and the
same defect.)

## Impact

A `neg ... ror #N` assembles without diagnostic to an instruction that does not
perform rotation — silent semantic corruption.

## Suggested fix

Reject unknown shift kinds (including `ror`) instead of defaulting to LSL:

```rust
let st = match kind.as_str() {
    "lsl" => 0b00u32,
    "lsr" => 0b01,
    "asr" => 0b10,
    other => return Err(format!("neg does not support shift '{}'", other)),
};
```
