# `encode_neon_two_misc` lets REV32 take word/double sizes (UNALLOCATED output)

**Witness:** `src/backend/arm/assembler/encoder/neon_eor_rev_pbt.rs`
`::rev32_rejects_word_or_double_arrangements` (`#[ignore]`d — reproduce with
`cargo test --lib neon_eor_rev -- --ignored rev32_rejects_word_or_double`).

## Minimal input

```
operands = [
    RegArrangement { reg: "v0", arrangement: "2s" },
    RegArrangement { reg: "v0", arrangement: "2s" },
]
encode_neon_two_misc(&operands, 1, 0b00000)   // the REV32 dispatch
```

## Expected vs actual

`REV32 (vector)` is defined by the ARMv8-A ARM **only** for
`size ∈ {00, 01}` (`.8B`/`.16B`/`.4H`/`.8H`). Sizes 10/11
(`.2S`/`.4S`/`.1D`/`.2D`) are UNALLOCATED.

```
$ echo 'rev32 v0.2s, v1.2s' | llvm-mc-18 -assemble -triple=aarch64
<stdin>:1:7: error: invalid operand for instruction
```

* **Expected:** `Err(...)` (size 10 is invalid for REV32).
* **Actual:** `Ok(0x2EA00800)` — `neon_arr_to_q_size("2s")` returns
  `(0, 0b10)` and the encoder emits it verbatim, producing an UNALLOCATED word.
  Same for `.4S`/`.1D`/`.2D`.

## Impact

Codegen that passes a `.2S`/`.4S`/`.1D`/`.2D` arrangement to REV32 emits
machine code a real assembler rejects / that traps as UNALLOCATED on hardware,
with no error at encode time.

## Fix

Same structural gap as REV16: `encode_neon_two_misc` only receives
`(u_bit, opcode)`, not the legal-size set. Pass a legal-size mask in, or add
an explicit arrangement guard (`arr ∈ {"8b","16b","4h","8h"}` /
`size ∈ {00,01}`) in the `mod.rs` `rev32` dispatch arm before encoding.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/313
