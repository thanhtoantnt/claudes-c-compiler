# Bug — `encode_orn` NEON path accepts non-byte and mismatched arrangements

**Function:** `encode_orn` (NEON vector branch) — `src/backend/arm/assembler/encoder/data_processing.rs`
(fn at line 910; NEON branch lines 919-924)
**PBT property:** `orn_neon_rejects_non_byte_or_mismatched_arrangement` — **FAILS**

## Reproduction

```
cargo test --lib 'data_processing::tests::orn_neon_rejects_non_byte_or_mismatched_arrangement'
```

Minimal failing input (proptest-shrunk): `rd = 0, rn = 0, rm = 0, bad = 0`
i.e. `orn v0.4h, v0.4h, v0.4h` is accepted and emitted with Q=0. The property also covers
`.2s`, `.1d`, `.8h`, and mismatched cases such as `orn v0.16b, v0.8b, v0.16b`.

## Spec (ARMv8 ARM §C7.2.2)

The bitwise logical *vector* instructions (AND, BIC, ORR, ORN, EOR, EON, BSL, BIT, BIF)
operate on byte elements only and are defined solely for the `.8b` (Q=0) and `.16b` (Q=1)
arrangements. Any other arrangement, or any mismatch between the three operands' arrangements,
is invalid. GAS rejects `orn v0.4h, v1.4h, v2.4h` with `Error: operand mismatch`.

## Root cause (`data_processing.rs:919-924`)

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;
let (rm, _) = get_neon_reg(operands, 2)?;
let q: u32 = if arr_d == "16b" { 1 } else { 0 };
```

The Q bit is derived only from operand 0's arrangement, and that derivation treats **every**
non-`"16b"` string as `.8b` (Q=0). There is no check that the arrangement is one of the two
legal byte arrangements, nor that the three arrangements are identical.

## Suggested fix

Validate the arrangement and require all three to match:
```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let (rm, arr_m) = get_neon_reg(operands, 2)?;
if !matches!(arr_d.as_str(), "8b" | "16b") || arr_n != arr_d || arr_m != arr_d {
    return Err("orn vector form requires .8b/.16b with matching arrangements".to_string());
}
let q: u32 = if arr_d == "16b" { 1 } else { 0 };
```

## Impact

`orn v0.4h, …` (and any non-byte / mismatched vector arrangement) assembles without error into
a word whose Q bit is wrong and which decodes as an `ORN Vd.8b, …` — a silent miscompilation
that is undetectable until runtime. The same `if arr_d == "16b" { 1 } else { 0 }` pattern (no
arrangement validation) recurs across many NEON encoders in this file/dir and likely harbours
the identical defect.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/86
