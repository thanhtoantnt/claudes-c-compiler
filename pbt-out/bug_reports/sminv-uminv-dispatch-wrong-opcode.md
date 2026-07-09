# Bug Report: `sminv` / `uminv` dispatched with wrong across-lanes opcode (`0b11010` instead of `0b01011`)

**Target:** `src/backend/arm/assembler/encoder/mod.rs` → instruction dispatch (callers of `encode_neon_across`)
**Severity:** High

## Summary

The assembler dispatch for `sminv` and `uminv` passes opcode `0b11010` to
`encode_neon_across`. Per the ARMv8-A ARM ("Advanced SIMD across lanes"), the
MINV instructions use opcode `0b01011`; `0b11010` is **not an allocated opcode**
in this group. As a result every emitted `sminv`/`uminv` is an UNALLOCATED
encoding that will UNDEF-trap at runtime.

## Root Cause

In `src/backend/arm/assembler/encoder/mod.rs` (~lines 707–710):

```rust
"umaxv" => encode_neon_across(operands, 1, 0b01010),  // CORRECT (UMAXV opc = 01010)
"uminv" => encode_neon_across(operands, 1, 0b11010),  // WRONG  — should be 0b01011
"smaxv" => encode_neon_across(operands, 0, 0b01010),  // CORRECT (SMAXV opc = 01010)
"sminv" => encode_neon_across(operands, 0, 0b11010),  // WRONG  — should be 0b01011
```

The MINV rows evidently copied the MAXV rows and flipped bit 4 of the opcode
(`01010` → `11010`) instead of bit 0 (`01010` → `01011`). The correct across-lanes
opcodes (ARMv8-A ARM) are:

| instr  | U | opcode (bits 16–12) |
|--------|---|----------------------|
| SMAXV  | 0 | `0 1 0 1 0` |
| UMAXV  | 1 | `0 1 0 1 0` |
| SMINV  | 0 | `0 1 0 1 1` |
| UMINV  | 1 | `0 1 0 1 1` |
| ADDV   | 0 | `1 1 0 1 1` |
| SADDLV | 0 | `0 0 0 1 0` |
| UADDLV | 1 | `0 0 0 1 0` |

## Reproduction

Assembling `sminv v0.4s, v1.4s` (the live dispatch path) yields a word with
opcode bits 16–12 = `11010`:

| instr | dispatched opcode | correct opcode | emitted word (v0.4s, v1.4s) | correct word |
|-------|-------------------|----------------|------------------------------|--------------|
| sminv | `0b11010`         | `0b01011`      | `0x4E99A820` (UNALLOCATED)   | `0x4EB0B820` |
| uminv | `0b11010`         | `0b01011`      | `0x6E99A820` (UNALLOCATED)   | `0x6EB0B820` |

(The "correct word" column is from the golden table in
`neon_across_pbt.rs::across_matches_golden_table`, computed independently from
the ARMv8-A ARM layout and using the correct `0b01011` opcode.)

## Impact

Every `sminv` and `uminv` instruction assembled by this backend produces an
UNALLOCATED encoding (`opcode = 0b11010`), which UNDEF-traps at runtime. This is
a silent mis-compilation affecting a whole instruction class. The sibling
`encode_neon_across` itself is correct — it faithfully places the supplied opcode
into bits 16–12 — so the defect is purely the caller's constant.

## Suggested Fix

```rust
"uminv" => encode_neon_across(operands, 1, 0b01011),  // was 0b11010
"sminv" => encode_neon_across(operands, 0, 0b01011),  // was 0b11010
```

## Regression Property

This is a *caller* defect; the property suite for `encode_neon_across`
(`neon_across_pbt.rs`) anchors the function's correctness with the **correct**
opcode `0b01011` in its golden table (`across_matches_golden_table`), so once
the dispatch is fixed the emitted words will match that oracle. A direct
end-to-end regression test would assemble `sminv v0.4s, v1.4s` through the
top-level `encode_instruction` dispatch and assert the result is `0x4EB0B820`.

```rust
// After fix: dispatch end-to-end must yield the golden word.
// sminv v0.4s, v1.4s  →  0x4EB0B820
// uminv v0.4s, v1.4s  →  0x6EB0B820
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/210
