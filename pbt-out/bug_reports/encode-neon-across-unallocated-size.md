# Bug Report: `encode_neon_across` accepts unallocated `size=0b11` arrangements (`.1d`/`.2d`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_across`
**Severity:** Medium

## Summary

`encode_neon_across` is the shared backend for the AArch64 "Advanced SIMD across
lanes" reduction instructions (`UMAXV` / `UMINV` / `SMAXV` / `SMINV` — and the
correct reference shape used by `ADDV` / `SADDLV` / `UADDLV`). It derives the
`(Q, size)` fields purely from the source arrangement via `neon_arr_to_q_size`,
which returns `Ok` for `1d`/`2d` with `size = 0b11`.

But the entire across-lanes group is defined by the ARMv8-A ARM **only** for
arrangements whose `size` is `0b00`, `0b01`, or `0b10` (`.8b/.16b`, `.4h/.8h`,
`.2s/.4s`). `size = 0b11` (the `.1d` / `.2d` arrangements) is **UNALLOCATED**
(UNDEFINED) for every instruction in this group. The function performs no
`size` validation, so it silently emits an UNDEF-trapping word instead of
returning `Err`.

## Root Cause

In `src/backend/arm/assembler/encoder/neon.rs`:

```rust
pub(crate) fn encode_neon_across(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON across-vector requires 2 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_n)?;   // <-- Ok for 1d/2d (size=0b11); never range-checked

    // 0 Q U 01110 size 11000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (0b11000 << 17)
        | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`neon_arr_to_q_size` maps `"1d" => Ok((0, 0b11))` and `"2d" => Ok((1, 0b11))`, so
`size = 0b11` flows straight into bits 23–22 of the word. No branch rejects it.

## Reproduction

```
encode_neon_across(&[v0.1d, v1.1d], 1, 0b01010)   // "umaxv v0.1d, v1.1d"
  → Ok(EncodeResult::Word(0x2EF0A820))             // bits 23-22 = 11  (UNALLOCATED)

encode_neon_across(&[v0.2d, v1.2d], 1, 0b01010)   // "umaxv v0.2d, v1.2d"
  → Ok(EncodeResult::Word(0x6EF0A820))             // bits 23-22 = 11  (UNALLOCATED)
```

Expected: `Err` (the arrangement is not valid for any across-lanes instruction).
Actual: `Ok(word)` with `size=0b11`, an encoding that is UNALLOCATED and would
UNDEF-trap at runtime.

The same defect reproduces for every opcode / U-bit combination the function
serves (`umaxv` / `uminv` / `smaxv` / `sminv` / `addv` / `saddlv` / `uaddlv`).

## Impact

The assembler accepts invalid mnemonics and produces UNDEF-trapping encodings
silently — no error, no warning. This is a silent correctness gap: an invalid
`umaxv v0.1d, v1.1d` is assembled as if valid. The same class of defect is
already documented for `ADDV` (`encode_neon_addv_accepts_unallocated_arrangements.md`).
No crash occurs in the assembler itself; the damage is a wrong machine-code word
shipped downstream.

## Suggested Fix

Reject `size == 0b11` immediately after resolving the arrangement:

```rust
let (q, size) = neon_arr_to_q_size(&arr_n)?;
if size == 0b11 {
    return Err(format!(
        "across-lanes reductions are not defined for arrangement {:?} (size=0b11 is UNALLOCATED)",
        arr_n));
}
```

(Audit `encode_neon_across_long` — the backend used for `saddlv`/`uaddlv` —
separately; the same `size=0b11` prohibition applies to the whole group.)

## Regression Property

Failing property: `across_rejects_unallocated_arrangements` (file
`src/backend/arm/assembler/encoder/neon_across_pbt.rs`, currently `#[ignore]`d).

```rust
#[test]
#[ignore]
fn across_rejects_unallocated_arrangements() {
    for arr in &["1d", "2d"] {
        let ops = vec![va(0, arr), va(1, arr)];
        let res = encode_neon_across(&ops, 1, 0b01010);
        assert!(res.is_err(),
            "across-lanes reductions do not support .{arr} (size=0b11 is UNALLOCATED); \
             expected Err but got Ok(0x{:08X})", word_of(res.clone()));
    }
}
```

Reproduce: `cargo test --lib neon_across_pbt::across_rejects_unallocated_arrangements -- --ignored`

---

## Related finding (caller, NOT this function) — `sminv`/`uminv` dispatched with wrong opcode

While testing `encode_neon_across`, the dispatch table in
`src/backend/arm/assembler/encoder/mod.rs` (~lines 707–710) was found to pass a
**wrong opcode** to this function for the MINV instructions:

```rust
"umaxv" => encode_neon_across(operands, 1, 0b01010),  // CORRECT
"uminv" => encode_neon_across(operands, 1, 0b11010),  // WRONG: should be 0b01011
"smaxv" => encode_neon_across(operands, 0, 0b01010),  // CORRECT
"sminv" => encode_neon_across(operands, 0, 0b11010),  // WRONG: should be 0b01011
```

Per the ARMv8-A ARM the MINV opcode is `0b01011`, not `0b11010` (`0b11010` is
unallocated in this group). `encode_neon_across` is innocent — it faithfully
places whatever opcode it is handed into bits 16–12 — so this is a separate
*caller* defect, documented in its own report:
`sminv-uminv-dispatch-wrong-opcode.md`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/209
