# Bug Report — `encode_neon_shift_right` silently truncates out-of-range immediates

**File:** `src/backend/arm/assembler/encoder/neon.rs`
**Function:** `encode_neon_shift_right(operands, u_bit, opcode)` (line ~1454)
**Found by:** property `shift_right_pbt_tests::prop_huge_immediate_not_truncated` (FAILS)

## Summary

The shift immediate is read as a signed 64-bit value and cast to `u32` **before** the
range check, so any `i64` immediate whose low 32 bits happen to fall in `1..=esize`
is silently accepted and encoded as if it were that small value, instead of being
rejected as out of range.

```rust
let shift = get_imm(operands, 2)? as u32;   // <-- truncation happens here
...
if shift == 0 || shift > element_bits { return Err(...); }  // checks the *truncated* value
```

`get_imm` returns the raw `i64` from `Operand::Imm`. The `as u32` narrows it, so the
range guard only ever sees the low 32 bits.

## Reproduction (minimal case, from proptest shrink)

Input immediate `4294967297` (`= 2^32 + 1`) with an `.8b` arrangement:

```text
srshr v0.8b, v1.8b, #4294967297
```

- Expected: `Err` (immediate is astronomically out of the valid `1..=8` range for bytes).
- Actual: `Ok(0x0F0F2420)` — i.e. it encodes exactly as `srshr v0.8b, v1.8b, #1`
  (the `as u32` cast reduces `4294967297` to `1`).

Any `base` with `1 <= base <= esize` and any `k >= 1` triggers it:
`shift = k * 2^32 + base` ⇒ cast to `base` ⇒ accepted.

## Impact

- Low likelihood in practice (the assembler frontend produces small literals for shift
  operands), but it defeats the otherwise-correct range validation and would let a
  pathological / attacker-crafted immediate produce a silently-wrong instruction.
- Severity: correctness/robustness gap (no crash), affects all 7 arrangements and all
  shift-right-by-immediate mnemonics dispatched here (srshr/urshr/ssra/usra/srsra/ursra).

## Suggested fix

Validate against the full `i64` value before truncation, e.g.:

```rust
let shift_i64 = get_imm(operands, 2)?;
if shift_i64 <= 0 || shift_i64 as u64 > element_bits as u64 {
    return Err(format!("shift-right: shift {} out of range for {}-bit elements",
                       shift_i64, element_bits));
}
let shift = shift_i64 as u32;
```

## Test coverage added

`src/backend/arm/assembler/encoder/neon.rs` → module `shift_right_pbt_tests`
(proptest, appended). Results:

| Property | Result |
|---|---|
| `prop_matches_reference` (differential oracle vs. independent ARM ARM reconstruction) | PASS |
| `prop_immhb_field` (immh:immb == 2*esize − shift) | PASS |
| `prop_fields_isolated` (Rd/Rn/opcode/fixed-bit placement) | PASS |
| `prop_out_of_range_rejected` (shift 0, shift > esize, negative, bad arrangement, arity) | PASS |
| `prop_huge_immediate_not_truncated` (i64 ≥ 2^32 with in-range low 32 bits must Err) | **FAIL** |

The first four properties confirm the encoder is bit-for-bit correct for every valid
input (all arrangements, all opcodes, both U values, full register range). The fifth
isolates and documents the truncation gap.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/73
