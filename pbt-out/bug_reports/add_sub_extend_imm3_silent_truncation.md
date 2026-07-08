# Bug — `encode_add_sub` silently truncates extended-register shift (`imm3`)

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, `encode_add_sub` (extended-register branch)
**Found by:** property `tests::extend_amount_above_7_must_be_rejected` (proptest, same file).
**Severity:** high — produces a *valid-but-wrong* encoding with no diagnostic.

## Where
```rust
if let Some(Operand::Extend { kind, amount }) = operands.get(3) {
    ...
    let imm3 = *amount & 0x7;   // <-- masks bits above the 3-bit imm3 field
    let word = ... | (option << 13) | (imm3 << 10) | (rn << 5) | rd;
    return Ok(EncodeResult::Word(word));
}
```

## Problem
The ARMv8 ARM *Add (extended register)* encoding places the optional shift into the
3-bit `imm3` field (bits 12:10). The field can only hold values `0..=7`; the
architecture further constrains the maximum per extend kind (e.g. for `uxtb`/`sxtb`
the shift must be `0..=4`). Any shift amount `≥ 8` is **unrepresentable** and
UNDEFINED. GAS and LLVM reject such inputs at assembly time.

The current code applies `& 0x7`, so e.g. `add x0, x1, x2, uxtw #8` silently encodes
as `uxtw #0`, `… #9` as `uxtw #1`, etc. — a different instruction from the one the
source requested, with no diagnostic. This is the worst failure mode for an encoder:
downstream consumers (assembler users, the linker, tests) cannot detect it.

## Reproducing property (fails)
`tests::extend_amount_above_7_must_be_rejected`
```text
minimal failing input: rd = 0, rn = 0, rm = 0, ek = 0, amount = 8
assertion failed: encode_add_sub(&ops, false, false).is_err()
```

## Suggested fix
Validate the shift amount before encoding:
```rust
let imm3 = *amount;
if imm3 > 7 {
    return Err(format!("extended-register shift {} out of range (0..=7)", imm3));
}
// (optional, stricter) enforce per-extend-kind maximums per ARMv8 ARM
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/1
