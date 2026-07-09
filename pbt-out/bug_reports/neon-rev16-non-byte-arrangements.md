# `encode_neon_two_misc` lets REV16 take non-byte sizes (UNALLOCATED output)

**Witness:** `src/backend/arm/assembler/encoder/neon_eor_rev_pbt.rs`
`::rev16_rejects_non_byte_arrangements` (`#[ignore]`d — reproduce with
`cargo test --lib neon_eor_rev -- --ignored rev16_rejects_non_byte`).

## Minimal input

```
operands = [
    RegArrangement { reg: "v0", arrangement: "4h" },
    RegArrangement { reg: "v0", arrangement: "4h" },
]
encode_neon_two_misc(&operands, 0, 0b00001)   // the REV16 dispatch
```

## Expected vs actual

`REV16 (vector)` is defined by the ARMv8-A ARM **only** for `size = 00`
(`.8B`/`.16B`). Sizes 01/10/11 (`.4H`/`.2S`/`.2D`, …) are UNALLOCATED.

```
$ echo 'rev16 v0.4h, v1.4h' | llvm-mc-18 -assemble -triple=aarch64
<stdin>:1:7: error: invalid operand for instruction
```

* **Expected:** `Err(...)` (size 01 is invalid for REV16).
* **Actual:** `Ok(0x0E601800)` — `neon_arr_to_q_size("4h")` returns
  `(0, 0b01)` and the encoder emits it verbatim, producing an UNALLOCATED word.
  Same for `.8H`/`.2S`/`.4S`/`.1D`/`.2D`.

## Impact

Codegen that passes a non-byte arrangement to REV16 emits machine code a real
assembler rejects / that traps as UNALLOCATED on hardware, with no error at
encode time.

## Fix

`encode_neon_two_misc` currently only receives `(u_bit, opcode)` — it has no
way to know which sizes are legal. Either pass a legal-size mask in (REV16:
`size == 00`; REV32: `size ∈ {00, 01}`; …) or validate in the `mod.rs`
dispatcher for the `rev16` arm:

```rust
"rev16" => if matches!(operands.first(), Some(Operand::RegArrangement { .. })) {
    // REV16 is size=00 only: .8b/.16b
    encode_neon_two_misc(operands, 0, 0b00001)  // + arrangement check
} else { encode_rev16(operands) },
```
with an explicit `arr ∈ {"8b","16b"}` guard (or a size check) before encoding.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/312
