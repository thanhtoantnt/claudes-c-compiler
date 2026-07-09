# Bug: `encode_neon_eor3` does not validate the `.16B` arrangement

| Field | Value |
|---|---|
| Function | `encode_neon_eor3` |
| File | `src/backend/arm/assembler/encoder/neon.rs:1112` |
| Severity | Correctness / silent corruption |
| Discovered by | property `rejects_non_canonical_arrangements` in `neon_eor3_pbt.rs` |
| Status | **Confirmed** (property fails; 5 sibling properties pass) |

## Summary

`EOR3` (FEAT_SHA3) is architecturally defined **only** as:

```
EOR3 Vd.16B, Vn.16B, Vm.16B, Va.16B
```

Its encoding (`11001110 sz 0 Rm 0 Ra Rn Rd`, sz=00) has **no Q bit** — bits 31–30
are fixed `11` — so an `.8B` form does not exist and every other arrangement
(`.4h/.8h/.2s/.4s/.1d/.2d`) is UNALLOCATED. LLVM's assembler rejects these with
`error: invalid operand for instruction`.

The crate's encoder discards the arrangement on **all four** operands:

```rust
let (rd, _) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;
let (rm, _) = get_neon_reg(operands, 2)?;
let (rk, _) = get_neon_reg(operands, 3)?;
```

Consequently it emits a valid-looking EOR3 word for **any** arrangement,
silently mis-encoding the instruction.

## Reproduction

```
cargo test --lib neon_eor3_pbt::rejects_non_canonical_arrangements
```

Minimal failing input reported by proptest:

```
arr = "8b"
successes: 0   local rejects: 0   global rejects: 0
```

The encoder returned:

```
Ok(Word(3456240672))   // = 0xCE020C20
```

`0xCE020C20` is byte-identical to the encoding of
`eor3 v0.16b, v1.16b, v2.16b, v3.16b` — i.e. the `.8b` input produced the
`.16B` instruction, with no error.

## Expected vs. actual

| Input | Expected | Actual |
|---|---|---|
| `eor3 v0.16b, v1.16b, v2.16b, v3.16b` | `Ok(Word(0xCE020C20))` | `Ok(Word(0xCE020C20))` ✓ |
| `eor3 v0.8b,  v1.8b,  v2.8b,  v3.8b`  | `Err(...)` (UNALLOCATED) | `Ok(Word(0xCE020C20))` ✗ |
| `eor3 v0.4s,  v1.4s,  v2.4s,  v3.4s`  | `Err(...)` (UNALLOCATED) | `Ok(Word(0xCE020C20))` ✗ |

## Suggested fix

Validate that the arrangement of all four operands is exactly `"16b"` before
encoding (mirroring `neon_arr_to_q_size` / the `arr_d == "16b"` checks used
elsewhere in the file, e.g. `encode_neon_logical`, `encode_neon_ext`):

```rust
for (i, op) in operands.iter().enumerate().take(4) {
    if !matches!(op, Operand::RegArrangement { arrangement, .. } if arrangement == "16b") {
        return Err(format!("eor3 requires .16b arrangement on all operands (operand {i})"));
    }
}
```

## Verification

After the fix, `rejects_non_canonical_arrangements` must pass while the other
five properties (golden table, reference-encoder differential, register-field
round-trip, fixed-bits invariant, operand-count error contract) continue to
pass.
