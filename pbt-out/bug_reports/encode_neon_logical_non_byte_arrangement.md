# `encode_neon_logical` accepts UNALLOCATED non-byte arrangements

## Summary
`encode_neon_logical` (`src/backend/arm/assembler/encoder/neon.rs:297`)
silently encodes `AND`/`ORR`/`EOR Vd.T, Vn.T, Vm.T` for arrangements that are
architecturally **UNALLOCATED** for the logical group (`.4h`, `.8h`, `.2s`,
`.4s`, `.1d`, `.2d`). It derives the `Q` bit solely from `arr == "16b"` and
treats every other arrangement string as `Q=0` (i.e. as `.8b`), emitting a
valid-looking byte word instead of returning `Err`.

## Architecture reference
`AND`/`ORR`/`EOR` (vector) are slots of the AArch64 "Advanced SIMD three same"
**logical** group (ARMv8-A ARM, ARM DDI 0487, AND/ORR/EOR rows):

```
  31 30 29 28-24 23-22 21 20-16 15-11 10 9-5 4-0
   0  Q  U  01110  size  1   Rm  00011   1  Rn  Rd
```

The `size` field is **fixed per op** (AND=00, ORR=10, EOR=00) and the whole
group is **byte-only**: only `.8b` (Q=0) and `.16b` (Q=1) are allocated; all
other arrangements are UNALLOCATED.

## Minimal failing input
```
AND v0.4h, v1.4h, v2.4h     -> encode_neon_logical(&ops, 0b00) -> Ok(0x0E221C20)
```

- **Expected:** `Err` ("invalid arrangement; logical ops require .8b/.16b").
- **Actual:** `Ok(0x0E221C20)` — which is the encoding of
  `and v0.8b, v1.8b, v2.8b`, silently corrupting the instruction.

LLVM's AArch64 assembler (`clang --target=aarch64`) rejects `.4h/.8h/.2s/.4s/
.1d/.2d` with "invalid operand for instruction".

## Code
```rust
// encode_neon_logical, neon.rs:297
let q: u32 = if arr_d == "16b" { 1 } else { 0 };   // <-- no arrangement validation
...
let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size_bits << 22)
    | (1 << 21) | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
```

## Impact
A caller reaching this path with a non-byte arrangement (e.g. a future codegen
lowering mistake) gets a silently-wrong, valid-looking instruction instead of a
diagnostic. Same bug class as the already-filed
`encode_neon_bic_non_byte_arrangement.md`.

## Witnessing test
`src/backend/arm/assembler/encoder/neon_logical_pbt.rs`,
`rejects_non_byte_arrangement` — `#[ignore]`d (default `cargo test` stays
green). Reproduce:
```
cargo test --package ccc neon_logical_pbt -- --ignored rejects_non_byte_arrangement
```

## Suggested fix
```rust
let q: u32 = match arr_d.as_str() {
    "8b" => 0,
    "16b" => 1,
    other => return Err(format!("NEON logical requires .8b/.16b, got .{other}")),
};
```
