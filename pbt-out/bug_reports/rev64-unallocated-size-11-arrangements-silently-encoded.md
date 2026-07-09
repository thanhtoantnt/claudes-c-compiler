# REV64 encoder bug report — unallocated `size=11` arrangements silently encoded

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_rev64`
**Property suite:** `src/backend/arm/assembler/encoder/neon_rev64_pbt.rs`
**Severity:** medium (silent emission of an architecturally-UNDEFINED instruction)

## Summary

`encode_neon_rev64` accepts the `.1d` and `.2d` arrangements and emits an
**UNALLOCATED** instruction word instead of returning `Err`. REV64 reverses
elements *within each 64-bit doubleword*, so the operation is meaningless for
64-bit elements: `size = 0b11` is UNDEFINED for this instruction.

The root cause is that arrangement parsing is delegated to
`neon_arr_to_q_size`, which maps `1d → (Q=0, size=0b11)` and
`2d → (Q=1, size=0b11)` with no check that `size == 0b11` is invalid for
REV64.

## Evidence

### 1. llvm-mc (reference assembler) rejects `.1d`/`.2d`

```
$ echo 'rev64 v0.8b, v1.8b'  | llvm-mc-18 -assemble -triple=aarch64 -show-encoding
rev64 v0.8b, v1.8b   // encoding: [0x20,0x08,0x20,0x0e]   = 0x0E200820
$ echo 'rev64 v0.1d, v1.1d'  | llvm-mc-18 -assemble -triple=aarch64
error: invalid operand for instruction
```

All six *valid* arrangements (8B/16B/4H/8H/2S/4S) match this encoder byte-for-byte;
the two invalid ones are rejected by llvm-mc but accepted here.

### 2. Failing property (`#[ignore]`d, but a genuine `proptest!` negative-contract property)

`rev64_rejects_unallocated_arrangements` is a `proptest!` property that asserts
every `size=11` arrangement (`.1d`/`.2d`) must return `Err`. It is `#[ignore]`d
to keep the default suite green (see *Design Caveats*); run it to reproduce:

```
$ cargo test --lib neon_rev64 -- --ignored rev64_rejects_unallocated_arrangements

minimal failing input: rd = 0, rn = 0, arr = "1d"
        successes: 0
        ...
panicked: ... size=11 is UNDEFINED for REV64 (llvm-mc rejects);
          expected Err but got Ok(Word(249563136))   # 0x0EE00820
test result: FAILED. 0 passed; 1 failed
```

proptest shrinks to the minimal counterexample `rev64 v0.1d, v0.1d`. The
persisted regression seed is `cc a9914e2d...51daa8` (`shrinks to rd=0, rn=0,
arr="1d"`). `0x0EE00820` decodes as `REV64 V0.<T>, V1.<T>` with `size=11` —
the combination the ARMv8-A ARM marks UNDEFINED/unallocated for this opcode.

## Architecture context (ARMv8-A ARM, "REV64 (vector)")

The valid `<T>` arrangements are **8B, 16B, 4H, 8H, 2S, 4S** only — element
sizes of 8, 16, and 32 bits. There is no `1D`/`2D` form; `size=0b11` is
reserved. Encoding for the valid forms (verified against llvm-mc):

| instruction        | word       |
|--------------------|------------|
| rev64 v0.8b, v1.8b  | 0x0E200820 |
| rev64 v0.16b, v1.16b| 0x4E200820 |
| rev64 v0.4h, v1.4h  | 0x0E600820 |
| rev64 v0.8h, v1.8h  | 0x4E600820 |
| rev64 v0.2s, v1.2s  | 0x0EA00820 |
| rev64 v0.4s, v1.4s  | 0x4EA00820 |

The fixed-field encoding itself is **correct**; the only defect is the missing
`size != 0b11` range check.

## Suggested fix

After `let (q, size) = neon_arr_to_q_size(&arr_d)?;`, reject 64-bit elements:

```rust
if size == 0b11 {
    return Err(format!("rev64: invalid arrangement {} (size=11 is UNDEFINED for REV64)", arr_d));
}
```

The same defect class affects other two-register-miscellaneous encoders that
share `neon_arr_to_q_size` and are also undefined for `size=11` (e.g. the
documented `ADDV` gap in `neon_addv_pbt.rs`).

## Test suite status

All six *passing* properties confirm the encoder is bit-exact correct for valid
input (golden table cross-validated with llvm-mc, differential vs. an
independent reference encoder, fixed-bits invariant, field round-trip, and
negative contract for unknown arrangements / operand counts). The one
`#[ignore]`d test documents this gap.
