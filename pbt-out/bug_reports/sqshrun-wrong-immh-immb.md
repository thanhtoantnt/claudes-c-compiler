# `encode_neon_sqshrun` — wrong `immh:immb` field

One confirmed bug: the `immh:immb` field of the encoded word is computed with
the wrong formula, so the emitted instruction decodes as a different (or
UNALLOCATED) operation.

## Status
Confirmed by the `#[ignore]`d witnesses in
`src/backend/arm/assembler/encoder/neon_sqshrun_pbt.rs`. They fail on the
current implementation:

```
$ cargo test --lib neon_sqshrun -- --ignored
test ...::immh_immb_matches_arm_spec ... FAILED        # immh:immb 0xF != 0x1F
test ...::golden_immh_immb_diverges_from_arm ... FAILED # 0x2F0F8420 != 0x2F1F8420
```

## Location
`src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_sqshrun`.

## Minimal input
`SQSHRUN V0.8b, V1.8h, #1` — i.e.
`operands = [V0.8b, V1.8h, #1]`, `is_rounding = false`, `is_high = false`.

- **Expected word:** `0x2F1F8420`  (`immh:immb = 0x1F`)
- **Actual word:**   `0x2F0F8420`  (`immh:immb = 0x0F`)
- **Difference:** only the `immh:immb` field (bits 22–16) is wrong; every other
  field (Q, U, the `0_011110` prefix, opcode `100001`, Rn, Rd) is correct.

## Root cause
```rust
let immhb = (element_bits - shift) & 0x7F; // BUG
let immh = (immhb >> 3) | immh_base;        // BUG: immh_base also wrong
let immb = immhb & 0x7;
```

`encode_neon_sqshrun` is in the AArch64 "Advanced SIMD shift by immediate"
group, whose decode maps `immh` to the source element size and recovers the
shift as `shift = 2*esize - immh:immb`, i.e. `immh:immb = 2*esize - shift`. The
crate already uses this correct formula in the non-narrow siblings
`encode_neon_ushr` / `encode_neon_shift_right` (`element_bits * 2 - shift`), but
`encode_neon_sqshrun` uses `element_bits - shift` (half the correct value) and
ORs in a too-small `immh_base` (`0b0001/0b0010/0b0100` instead of
`0b0010/0b0100/0b1000`).

## Impact
The emitted `immh:immb` corresponds to an element size one level smaller than
the source, so the word decodes as a different instruction (or, for `.8h`
sources, an UNALLOCATED encoding). This is silent code corruption: there is no
error returned and the register/opcode fields look valid. Every other field is
correct; only `immh:immb` is broken.

## Fix
```rust
let immhb = element_bits * 2 - shift;   // immh:immb = 2*esize - shift
let immh = (immhb >> 3) & 0xF;          // immh_base no longer needed
let immb = immhb & 0x7;
```

## Verification note
No AArch64 assembler (`llvm-mc` / `aarch64-linux-gnu-as`) is available in this
environment, so the oracle is the field-correct reference encoder in the test
file. The reference deliberately shares the implementation's `U=1` and `opcode`
bits so the differential isolates the `immh:immb` bug exactly. The `U` bit
(bit 29 = 1) and the exact opcode bits were not independently verified against
LLVM here; this finding does not depend on them.
