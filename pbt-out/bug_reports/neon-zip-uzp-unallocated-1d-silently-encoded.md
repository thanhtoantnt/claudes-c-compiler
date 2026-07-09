# Bug Report: `encode_neon_zip_uzp` silently encodes the unallocated `.1d` arrangement

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_zip_uzp`

## Summary

`encode_neon_zip_uzp` encodes the AArch64 NEON permute family
(`UZP1/UZP2/ZIP1/ZIP2/TRN1/TRN2`). For the arrangement `.1d` it silently emits
an **UNALLOCATED** instruction word instead of returning `Err`, because it
accepts the `size=11, Q=0` combination that the ARMv8-A ARM marks UNDEFINED
for this instruction class.

## Architecture reference

`UZP1/UZP2/ZIP1/ZIP2/TRN1/TRN2` live in the "Advanced SIMD three same" group,
permute sub-class:

```text
  31 30 29 28-24 23-22 21  20-16 15 14-12  11-10 9-5 4-0
   0  Q  0  01110  size  0  Rm    0  opcode  10   Rn  Rd
```

The ARMv8-A ARM defines these instructions for arrangements `8B/16B/4H/8H/
2S/4S/2D` — i.e. `(size,Q)` ∈ `{(00,*),(01,*),(10,*),(11,1)}`. The combination
`size=11, Q=0` (the `.1d` arrangement) is **UNALLOCATED / UNDEFINED**. LLVM's
assembler rejects it outright:

```text
  $ echo 'uzp1 v0.1d, v1.1d, v2.1d' | llvm-mc-18 -assemble -triple=aarch64
  <stdin>:1:6: error: invalid operand for instruction
```

`.2d` (size=11, Q=1) **is** valid and assembles correctly.

## Reproduction

```rust
use crate::backend::arm::assembler::encoder::encode_neon_zip_uzp;
use crate::backend::arm::assembler::parser::Operand;

let ops = vec![
    Operand::RegArrangement { reg: "v0".into(), arrangement: "1d".into() },
    Operand::RegArrangement { reg: "v1".into(), arrangement: "1d".into() },
    Operand::RegArrangement { reg: "v2".into(), arrangement: "1d".into() },
];
let res = encode_neon_zip_uzp(&ops, 0b001, false); // uzp1
// Expected: Err(...)
// Actual:   Ok(EncodeResult::Word(0x0EC01800))   <- UNALLOCATED encoding
```

## Root cause

`encode_neon_zip_uzp` derives `(Q, size)` solely via `neon_arr_to_q_size`,
which maps `1d` → `(Q=0, size=0b11)` without any class-specific check. No test
excludes the `size=11 && Q==0` case, so the resulting word is emitted as-is:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
let word = (((q << 30) | (0b001110 << 24) | (size << 22)) | (rm << 16))
         | (op_bits << 12) | (0b10 << 10) | (rn << 5) | rd;
```

## Impact

A user can write `uzp1 v0.1d, v1.1d, v2.1d` (and the ZIP/TRN/UZP2 variants) and
receive a silently-malformed 32-bit word. The output is not a valid
instruction (UNALLOCATED), so downstream consumption (execution / disassembly)
is undefined. This is a negative-contract violation: an out-of-range / reserved
arrangement should be rejected.

## Suggested fix

In `encode_neon_zip_uzp`, after computing `(q, size)`, reject `size==0b11 &&
q==0`:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 && q == 0 {
    return Err(format!("permute instructions do not support .1d arrangement"));
}
```

(Generalising `neon_arr_to_q_size` to carry validity per-instruction-class
would also fix the sibling findings in `encode_neon_rev64`,
`encode_neon_cmp_zero`, and others — see
`rev64-unallocated-size-11-arrangements-silently-encoded.md` and
`neon-cmp-zero-unallocated-size-11.md`.)

## Test

`src/backend/arm/assembler/encoder/neon_zip_uzp_pbt.rs`, `#[ignore]`d test
`rejects_unallocated_1d_arrangement` reproduces the failure:

```
cargo test --lib neon_zip_uzp -- --ignored rejects_unallocated_1d_arrangement
# FAILED: expected Err but got Ok(Word(0x0EC01800))
```

## Related finding (non-bug, documented)

The `is_zip: bool` parameter is unused (bound as `_is_zip`); ZIP/UZP/TRN are
differentiated only by `op_bits`. This is harmless but dead API surface,
captured by the passing property `is_zip_does_not_affect_encoding`.
