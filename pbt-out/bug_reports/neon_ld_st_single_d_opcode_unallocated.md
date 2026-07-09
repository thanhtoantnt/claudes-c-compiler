# `encode_neon_ld_st_single` — `.d` element emits an *unallocated* opcode

- **Module:** `src/backend/arm/assembler/encoder/neon.rs`
- **Function:** `encode_neon_ld_st_single`
- **Witness test:** `src/backend/arm/assembler/encoder/neon_ld_st_single_pbt.rs` — `d_element_matches_reference` (`#[ignore]`d `proptest!`; default `cargo test` stays green)
- **Reproduce:** `cargo test --lib neon_ld_st_single_pbt -- --ignored d_element_matches_reference`
- **Shrunk counterexample (Falsifiable):** `num=1, rt=0, rn=0, index=0, is_load=false, post=false` ⇒ `st1 {v0.d}[0], [x0]` → impl `0x0D008400` (opcode `100`, size `01`) vs spec `0x0D004400` (opcode `010`, size `01`)

## Symptom

`encode_neon_ld_st_single` produces words that are **not a valid AArch64
encoding** for 64-bit-element single-structure load/store.

```
st1 {v0.d}[0], [x0]            → impl: 0x0D008400   spec: 0x0D004400
ld3 {v2.d, v3.d, v4.d}[1], [x5] → impl opcode 101    spec opcode 011
```

## Root cause

In the `"d"` arm of the element-size `match`, the opcode is taken from the
`.s` group:

```rust
"d" => {
    let base_opc = if num_structs <= 2 { 0b100u32 } else { 0b101u32 }; // ← wrong
    let q = index & 1;
    (base_opc, 0u32, q, 0b01u32)
}
```

Per the ARMv8-A ARM ("Load/store SIMD &FP single structure"), the opcode field
(bits 15:13) selects the element-size class:

| elem | opcode 1/2 regs | opcode 3/4 regs | size      |
|------|-----------------|-----------------|-----------|
| `.b` | `000`           | `001`           | idx[1:0]  |
| `.h` | `010`           | `011`           | idx[0]:0  |
| `.s` | `100`           | `101`           | `00`      |
| `.d` | `010`           | `011`           | `01`      |

`.d` shares the **same opcode group as `.h` (`010`/`011`)** and is
distinguished from `.h` by `size[0] = 1` (size = `01`). The implementation
instead uses the `.s` opcodes (`100`/`101`), so the emitted word has
`opcode ∈ {100,101}` with `size = 01` — an **unallocated** combination.

## Why this is a bug, not a reference error

The independent reference encoder (`ref_encode_single` in the test file) is
assembled purely from the ARM ARM bit layout. It agrees with the
implementation on **all** `.b`/`.h`/`.s` cases — verified by the passing
differential property `matches_reference_encoder` over hundreds of random
`(elem, num_structs, index, regs, load/store, post-index)` cases. The
opcode-mapping table it encodes is therefore validated end-to-end; only the
`.d` case diverges, exactly where the implementation hard-codes `100/101`.

This is a textbook copy-paste defect: the `.d` arm looks like it was cloned
from the `.s` arm (`0b100`/`0b101`) without changing the opcode, while
correctly setting `size = 0b01` (which only pairs validly with opcode
`010`/`011`).

## Suggested fix

```rust
"d" => {
    let base_opc = if num_structs <= 2 { 0b010u32 } else { 0b011u32 }; // ← was 100/101
    let q = index & 1;
    (base_opc, 0u32, q, 0b01u32)
}
```



**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/318
