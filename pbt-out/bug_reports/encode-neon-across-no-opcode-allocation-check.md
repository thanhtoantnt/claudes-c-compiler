# Bug Report: `encode_neon_across` performs no opcode-allocation validation (emits UNALLOCATED opcodes)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_across`
**Severity:** Medium

## Summary

`encode_neon_across(operands, u_bit, opcode)` is the shared backend for the
AArch64 "Advanced SIMD across lanes" reductions (`UMAXV / UMINV / SMAXV / SMINV`).
It takes `opcode` as a caller-supplied parameter and places it verbatim into bits
16–12 of the word — **without checking that `opcode` is one of the opcodes
allocated to the across-lanes group**. The ARMv8-A ARM allocates only four opcode
field values in this group (`00010`, `01010`, `01011`, `11011`); every other
5-bit value is UNALLOCATED. The function will therefore emit UNALLOCATED
encodings for arbitrary caller inputs.

This is the root-cause gap that lets the `sminv`/`uminv` dispatch defect
(`sminv-uminv-dispatch-wrong-opcode.md`) manifest silently: a whitelist check
inside the backend would have caught the wrong `0b11010` opcode at encode time.

## Root Cause

In `src/backend/arm/assembler/encoder/neon.rs:445`:

```rust
pub(crate) fn encode_neon_across(operands: &[Operand], u_bit: u32, opcode: u32) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("NEON across-vector requires 2 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_n)?;

    // 0 Q U 01110 size 11000 opcode 10 Rn Rd
    let word = (q << 30) | (u_bit << 29) | (0b01110 << 24) | (size << 22) | (0b11000 << 17)
        | (opcode << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`(opcode << 12)` is unguarded: there is no check that `opcode ∈ {00010, 01010,
01011, 11011}`. (Note: `u_bit` is likewise unguarded, but `u_bit` is a genuine
per-instruction sign bit and not itself "allocated," so this report concerns only
the opcode field.) By contrast, `size` is range-checked in the sibling finding
`encode-neon-across-unallocated-size.md`; the opcode field has no equivalent
guard.

## Reproduction

```
encode_neon_across(&[v0.4s, v1.4s], 0, 0b00000)
  → Ok(EncodeResult::Word(0x4EB00820))   // opcode bits 16-12 = 00000  (UNALLOCATED)

encode_neon_across(&[v0.4s, v1.4s], 1, 0b11111)
  → Ok(EncodeResult::Word(0x6E9FF820))   // opcode bits 16-12 = 11111  (UNALLOCATED)
```

The allocated opcodes for this group (ARMv8-A ARM, "Advanced SIMD across lanes"):

| instr  | opcode (bits 16-12) |
|--------|----------------------|
| SADDLV / UADDLV | `0 0 0 1 0` |
| SMAXV / UMAXV   | `0 1 0 1 0` |
| SMINV / UMINV   | `0 1 0 1 1` |
| ADDV            | `1 1 0 1 1` |

Expected: `Err` for any `opcode ∉ {00010, 01010, 01011, 11011}`.
Actual: `Ok(word)` with the unallocated opcode placed in bits 16–12.

## Impact

Invalid/unallocated across-lanes opcodes assemble silently to UNDEF-trapping
encodings. This is defensive-depth gap: a backend-level whitelist would catch
caller bugs (such as the live `sminv`/`uminv` `0b11010` mistake) at encode time
instead of letting them ship. No crash in the assembler itself; the harm is a
silent mis-compilation.

## Suggested Fix

Whitelist the allocated opcodes after resolving the arrangement:

```rust
const ALLOCATED_ACROSS_OPCODES: &[u32] = &[0b00010, 0b01010, 0b01011, 0b11011];
if !ALLOCATED_ACROSS_OPCODES.contains(&opcode) {
    return Err(format!(
        "encode_neon_across: opcode 0b{opcode:05b} is not allocated in the across-lanes group"));
}
```

## Regression Property

Failing property: `across_rejects_unallocated_opcodes` (file
`src/backend/arm/assembler/encoder/neon_across_pbt.rs`, currently `#[ignore]`d).

```rust
#[test]
#[ignore]
fn across_rejects_unallocated_opcodes() {
    const ALLOCATED: &[u32] = &[0b00010, 0b01010, 0b01011, 0b11011];
    for opcode in 0u32..=0x1F {
        if ALLOCATED.contains(&opcode) { continue; }
        for &u_bit in &[0u32, 1] {
            let ops = vec![va(0, "4s"), va(1, "4s")];
            let res = encode_neon_across(&ops, u_bit, opcode);
            assert!(res.is_err(),
                "across-lanes opcode=0b{opcode:05b} (U={u_bit}) is UNALLOCATED; expected Err");
        }
    }
}
```

Reproduce: `cargo test --lib neon_across_pbt::across_rejects_unallocated_opcodes -- --ignored`

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/204
