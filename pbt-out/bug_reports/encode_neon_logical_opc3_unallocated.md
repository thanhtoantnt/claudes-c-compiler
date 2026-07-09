# `encode_neon_logical` silently aliases UNALLOCATED `opc=0b11` to `EOR`

## Summary
`encode_neon_logical` (`src/backend/arm/assembler/encoder/neon.rs:297`) maps the
`opc` selector to `(U, size)` for the NEON vector logical group. The arm for
`opc=0b11` is commented *"ANDS - not valid for NEON, fall back"* and returns
`Ok(...)` with `(U=1, size=0b00)` — **the exact encoding of `EOR` (`opc=0b10`)**.
There is no NEON vector logical op in the `0b11` slot (it is UNALLOCATED; "ANDS"
is an *integer* logical op, not NEON), so this should return `Err` rather than
silently emit an EOR word.

## Architecture reference
AArch64 "Advanced SIMD three same" logical group (ARMv8-A ARM, ARM DDI 0487):

```
  31 30 29 28-24 23-22 21 20-16 15-11 10 9-5 4-0
   0  Q  U  01110  size  1   Rm  00011   1  Rn  Rd
```

| opc  | op  | U  | size |
|------|-----|----|------|
| 0b00 | AND | 0  | 00   |
| 0b01 | ORR | 0  | 10   |
| 0b10 | EOR | 1  | 00   |
| 0b11 | —   | UNALLOCATED (no NEON vector logical op) |

## Minimal failing input
```
encode_neon_logical(&[v0.8b, v0.8b, v0.8b], 0b11) -> Ok(0x2E201C00)
```

- **Expected:** `Err` ("opc=0b11 is UNALLOCATED for NEON logical ops").
- **Actual:** `Ok(0x2E201C00)` — identical to `encode_neon_logical(&ops, 0b10)`
  (EOR). A caller passing the undefined `opc=0b11` would silently get an EOR.

## Code
```rust
// encode_neon_logical, neon.rs:297
let (u_bit, size_bits): (u32, u32) = match opc {
    0b00 => (0, 0b00),  // AND
    0b01 => (0, 0b10),  // ORR
    0b10 => (1, 0b00),  // EOR
    0b11 => (1, 0b00),  // ANDS - not valid for NEON, fall back   <-- aliased to EOR
    _ => return Err("unsupported NEON logical opc".to_string()),
};
```

## Impact
Low severity / currently unreachable. The only live caller
(`encode_logical` in `data_processing.rs:462`) forwards `opc` from instruction
dispatch, which never passes `0b11` for the NEON vector form. The pre-existing
in-file module `neon_logical_tests` (neon.rs:2908) characterises the aliasing
as a "known quirk". It is reported here because it is a reserved/unallocated
encoding that is silently miscoded rather than rejected; if a future change
ever routes `0b11` here it would produce a wrong instruction with no diagnostic.

## Witnessing test
`src/backend/arm/assembler/encoder/neon_logical_pbt.rs`,
`opc_3_is_unallocated` — `#[ignore]`d (default `cargo test` stays green).
Reproduce:
```
cargo test --package ccc neon_logical_pbt -- --ignored opc_3_is_unallocated
```

## Suggested fix
```rust
let (u_bit, size_bits): (u32, u32) = match opc {
    0b00 => (0, 0b00),  // AND
    0b01 => (0, 0b10),  // ORR
    0b10 => (1, 0b00),  // EOR
    _ => return Err(format!("UNALLOCATED/unsupported NEON logical opc: {opc}")),
};
```
