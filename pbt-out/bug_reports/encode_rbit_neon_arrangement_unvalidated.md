# Bug: `encode_rbit` silently accepts invalid NEON vector arrangements

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs`, `encode_rbit`
**Severity:** Correctness (silent mis-assembly of the NEON `RBIT` mnemonic)
**Found by:** `prop_encode_rbit_tests::prop_rejects_invalid_vector_arrangements` (property-based test, fails)

## Summary

The NEON (vector) branch of `encode_rbit` performs **no arrangement validation**.
It derives the `Q` bit with `q = if arr_d == "16b" { 1 } else { 0 }` and emits the
fixed byte-op word verbatim, so every arrangement other than `16b` is silently
encoded as the **8B** `RBIT` instruction, regardless of the mnemonic's arrangement
suffix. The source register's arrangement (`operands[1]`) is ignored entirely.

## Root cause

```rust
// NEON vector form: RBIT Vd.T, Vn.T (reverse bits in each byte)
if let Some(Operand::RegArrangement { .. }) = operands.first() {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;            // <-- source arrangement discarded
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };     // <-- no validation; defaults to 8B
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22)
        | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
    return Ok(EncodeResult::Word(word));
}
```

`get_neon_reg` returns the arrangement string but never validates it, and
`encode_rbit` (unlike `encode_rev32` in the same file) does **not** call
`neon_arr_to_q_size`, so there is no check that the arrangement is one of the
architecturally-allocated values.

## Architectural constraint (ARM ARM)

`RBIT` (vector) reverses the bits in each byte, so it has **no `size` field** and is
**constrained to `<T> ∈ {8B, 16B}`** only:

```
RBIT <Vd>.<T>, <Vn>.<T>     T = 8B, 16B
0 Q 1 01110 01 10000 00101 10 Rn Rd
```

Every other arrangement (`.4h`, `.8h`, `.2s`, `.4s`, `.1d`, `.2d`) is **unallocated**
and must be rejected with a diagnostic.

## Reproduction

Minimal failing input found by proptest:

```
mnemonic+operands: RBIT v0.4h, v0.4h
expected:          Err("invalid arrangement ...")
actual:            Ok(0x2E605800)     // == the 8B RBIT encoding
```

i.e. `RBIT v0.4h` is silently assembled as `RBIT v0.8b`. The same Ok-and-mis-encode
happens for `.8h`, `.2s`, `.4s`, `.1d`, `.2d` (all collapse to the 8B word). A
mismatched source such as `RBIT v0.16b, v1.8b` is also silently accepted (it uses the
destination's `16b` → `Q=1` and discards the source's `8b`).

## Properties added (`prop_encode_rbit_tests`)

| # | Property | Result |
|---|----------|--------|
| A | `prop_scalar_field_placement` — scalar fixed bits + field reconstruction | ✅ pass |
| B | `prop_scalar_matches_arm_reference` — full-word oracle (0x5AC00000 / 0xDAC00000), both widths | ✅ pass |
| C | `prop_neon_matches_arm_reference` — full-word oracle (0x2E605800 / 0x6E605800), 8B/16B | ✅ pass |
| D | `prop_differentials` — scalar X↔W differs only in sf; vector 8B↔16B only in Q | ✅ pass |
| E | `prop_rejects_malformed_operands` — missing/wrong-typed operands → Err | ✅ pass |
| F | `prop_rejects_invalid_vector_arrangements` — non-{8B,16B} & mismatched arrangements → Err | ❌ **FAIL (the bug)** |

Note: the **scalar** form of `encode_rbit` is correct for both widths (`sf` tracks the
register width, reference words match). The defect is confined to the NEON branch.

## Suggested fix

In the NEON branch, validate the arrangement against `{8B, 16B}` and require the
source and destination arrangements to match, e.g.:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("RBIT (vector) requires .8b or .16b, got .{}", arr_d));
}
if arr_d != arr_n {
    return Err(format!("RBIT (vector) arrangement mismatch: .{} vs .{}", arr_d, arr_n));
}
let q: u32 = if arr_d == "16b" { 1 } else { 0 };
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/150
