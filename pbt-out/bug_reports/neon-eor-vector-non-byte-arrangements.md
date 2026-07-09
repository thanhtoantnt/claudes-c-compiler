# `encode_neon_logical` (EOR vector) silently encodes non-byte arrangements as UNALLOCATED

**Witness:** `src/backend/arm/assembler/encoder/neon_eor_rev_pbt.rs`
`::eor_rejects_non_byte_arrangements` (`#[ignore]`d — reproduce with
`cargo test --lib neon_eor_rev -- --ignored eor_rejects_non_byte`).

## Minimal input

```
operands = [
    RegArrangement { reg: "v0", arrangement: "4h" },
    RegArrangement { reg: "v0", arrangement: "4h" },
    RegArrangement { reg: "v0", arrangement: "4h" },
]
encode_neon_logical(&operands, 0b10)   // opc 0b10 == EOR (vector)
```

## Expected vs actual

`EOR (vector)` (the `encode_logical → encode_neon_logical(opc=0b10)` path) is
defined by the ARMv8-A ARM **only** for `.8B`/`.16B` (bitwise op on byte
vectors). Every other arrangement must be rejected.

```
$ echo 'eor v0.4h, v1.4h, v2.4h' | llvm-mc-18 -assemble -triple=aarch64
<stdin>:1:5: error: invalid operand for instruction
```

* **Expected:** `Err(...)` (invalid arrangement for EOR).
* **Actual:** `Ok(0x2E201C00)` — the encoder sets `Q = (arr == "16b")` and
  forces `size = 00`, so `.4H` is silently remapped to a `.8B`-shaped word
  (identical to `eor v0.8b, v0.8b, v0.8b`). Same for `.8H`/`.2S`/`.4S`/
  `.1D`/`.2D`.

## Impact

Codegen that passes a non-byte arrangement to EOR (vector) emits machine code
a real assembler rejects / that traps as UNALLOCATED on hardware, with no
error at encode time.

## Fix

At the top of `encode_neon_logical`, validate the arrangement:

```rust
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("EOR (vector): invalid arrangement {arr_d:?}; only .8b/.16b allowed"));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/311
