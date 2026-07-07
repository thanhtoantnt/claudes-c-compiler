# Bug Report: `encode_movz` silently accepts out-of-range immediates and shift amounts

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_movz`

## Summary

`encode_movz` performs no range validation on either the 16-bit immediate or the `lsl`
shift amount. It silently **masks** the immediate (`& 0xFFFF`) and silently **normalizes**
the shift via integer division (`amount / 16`), returning a well-formed but semantically
wrong instruction word instead of an error.

This diverges from every reference AArch64 assembler (GAS, `llvm-mc`, ARM `armasm`), all of
which reject these inputs with messages like *"immediate out of range"* /
*"shift amount must be a multiple of 16"*.

## Reproduction

Two PBT properties fail (minimal failing inputs shown):

### Finding 1 — Immediate magnitude not validated

```text
movz_rejects_out_of_range_immediate
minimal failing input: rd = 0, imm = 65536
```

`movz x0, #0x10000` encodes `imm16 = 0x10000 & 0xFFFF = 0x0000`, i.e. it produces the
same word as `movz x0, #0x0`. Any upper bits are silently discarded.

Relevant source:
```rust
let imm = get_imm(operands, 1)?;
...
let word = (sf << 31) | (0b10100101 << 23) | (hw << 21)
         | (((imm as u32) & 0xFFFF) << 5) | rd;   // <-- silent mask, no Err
```

### Finding 2 — Shift amount not validated

```text
movz_rejects_non_multiple_of_16_shift
minimal failing input: rd = 0, hw = 0, rem = 1   (amount = 1)
```

`movz x0, #1, lsl #1` encodes `hw = 1 / 16 = 0`, i.e. it produces the same word as
`movz x0, #1` (no shift). `lsl #17` ⇒ `hw = 1` (treated as `lsl #16`). ARMv8 permits only
`lsl #{0, 16, 32, 48}` for MOVZ; any other amount is UNDEFINED.

Relevant source:
```rust
let hw = if operands.len() > 2 {
    if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
        if kind == "lsl" {
            *amount / 16          // <-- integer division, no Err
        } else {
            0                      // <-- non-lsl shifts silently ignored
        }
    } else { 0 }
} else { 0 };
```

A related sub-issue: a **non-`lsl`** shift kind (e.g. `lsr`, `asr`, `ror`) is silently
treated as `hw = 0` (no shift) rather than rejected. MOVZ only supports `LSL`.

### Finding 3 — 32-bit (`W`) register accepts `lsl #32` / `lsl #48` (UNDEFINED)

```text
movz_w_reg_rejects_32_or_48_shift   (proptest)
movz_w_reg_lsl32_is_rejected         (one-shot)
minimal failing input: rd = 0, bad_amount = 32
```
`movz w0, #1, lsl #32` encodes `hw = 32 / 16 = 2`. For a 32-bit MOVZ only
`hw ∈ {0, 1}` (i.e. `lsl #0`, `lsl #16`) is architecturally valid; `hw = 2`/`hw = 3`
produce UNDEFINED encodings. The `hw` computation ignores `is_64` entirely — the
same `*amount / 16` at `data_processing.rs:219` has no width gate.

### Systemic note

The identical masking (`& 0xFFFF`) and shift-normalization (`amount / 16`)
patterns are duplicated in `encode_movk` (`data_processing.rs:234`-`262`)
and `encode_movn` (`data_processing.rs:266`-`285`). All three MOV-wide encoders
share the same three defects.

## Impact

- **Silent miscompilation.** `movz x0, #0x10001` assembles to `mov x0, #1` with no
  diagnostic. Code that "assembles cleanly" executes with a wrong constant.
- **Width-invariant `hw`.** For 32-bit `MOVZ Wd`, only `hw ∈ {0,1}` is valid, yet the
  encoder happily emits `hw = 2`/`hw = 3` for `lsl #32`/`lsl #48` on a `W` register — an
  UNDEFINED encoding (the property suite constrains `hw ∈ 0..=3` for the 64-bit path only,
  so this case is not yet pinned by a test, but it falls out of the same root cause).
- Bugs of this shape are dangerous precisely because they never raise — downstream codegen
  produces an *apparently valid* instruction stream.

## Suggested fix

Validate before encoding:

```rust
let imm = get_imm(operands, 1)?;
if !(0..=0xFFFF).contains(&imm) {
    return Err(format!("movz immediate out of range: {}", imm));
}

let hw = match operands.get(2) {
    Some(Operand::Shift { kind, amount }) => {
        if kind != "lsl" {
            return Err(format!("movz only supports lsl shift, got {}", kind));
        }
        // 32-bit (W) MOVZ permits only hw in {0,1}; 64-bit (X) permits {0,1,2,3}.
        match (*amount, is_64) {
            (0, _)     => 0,
            (16, _)    => 1,
            (32, true) => 2,
            (48, true) => 3,
            _ => return Err(format!(
                "movz lsl shift {} invalid for {}-bit register", amount,
                if is_64 { 64 } else { 32 })),
        }
    }
    _ => 0,
};
```

This fix simultaneously addresses Findings 2 and 3 (the `is_64` gate on
`hw = 2`/`hw = 3`), and the magnitude check addresses Finding 1. The same
validation should be factored into a shared helper and applied to
`encode_movk` / `encode_movn` (see systemic note).

Then `(imm & 0xFFFF)` and `(hw << 21)` can remain as-is because the inputs are now
guaranteed in range.
