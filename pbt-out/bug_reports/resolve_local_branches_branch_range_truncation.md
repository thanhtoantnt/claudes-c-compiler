# Bug Report — `resolve_local_branches` silently truncates out-of-range branch offsets

**File:** `src/backend/arm/assembler/elf_writer.rs`
**Function:** `ElfWriter::resolve_local_branches`
**Severity:** High (silent miscompilation — wrong branch target, no diagnostic)

## Summary

When resolving a same-section branch relocation, `resolve_local_branches` patches the
instruction's immediate field by **masking** the PC-relative offset into the available
bit width, with **no range validation**. If the offset exceeds the instruction's
encodable range, the high bits are silently dropped (the value wraps), producing an
instruction that branches to a completely different target than requested. The function
returns `Ok(())` and emits no external relocation, so the corruption is invisible.

This violates the AArch64 ELF ABI range contract and the behavior of GAS, which errors
at assembly time (e.g. `Error: conditional branch out of range`) rather than emitting a
silently-wrapped encoding.

## Reproduction

Property `prop_branch_out_of_range_must_not_silently_truncate` (minimal input):

```
reloc_type = 280 (R_AARCH64_CONDBR19, encodable range ±1 MiB)
reloc.offset = 4          (branch lives at .text+4)
target       = ".text"+4  (label resolves to the same offset)
addend       = 2_000_000
```

Computed `pc_offset = target_offset - reloc.offset + addend = 2_000_000` (≈1.9 MiB,
out of the ±1 MiB CONDBR19 range).

Inside the function:

```rust
let pc_offset = (target_offset as i64) - (reloc.offset as i64) + reloc.addend; // 2_000_000
...
280 => { // R_AARCH64_CONDBR19
    let imm19 = ((pc_offset >> 2) as u32) & 0x7FFFF;   // 500_000, bit 18 set
    word |= imm19 << 5;                                 // silently wrapped
}
```

Decoding the patched word back: `imm19 = 500_000`, sign-extended from 19 bits →
`500_000 - 524_288 = -24_288`, so the encoded branch target is
`-24_288 * 4 = -97_152` bytes.

**Result:** a branch intended to reach `+2_000_000` bytes silently encodes a branch to
`-97_152` bytes. `resolve_local_branches` returns `Ok(())`; no relocation is emitted.

## Affected code paths

The masking-without-validation pattern is identical for every relocation type handled
in the same-section branch:

| reloc_type | field        | mask       | encodable range | code                       |
|-----------:|--------------|------------|-----------------|----------------------------|
| 282/283    | imm26 (B/BL) | `0x3FFFFFF`| ±128 MiB        | JUMP26 / CALL26            |
| 280        | imm19        | `0x7FFFF`  | ±1 MiB          | CONDBR19 (B.cond, etc.)    |
| 279        | imm14        | `0x3FFF`   | ±32 KiB         | TSTBR14 (TBZ/TBNZ)         |
| 273        | imm19        | `0x7FFFF`  | ±1 MiB          | LD_PREL_LO19 (LDR literal) |
| 274        | immlo/hi 21  | `0x3`/`0x7FFFF` | ±1 MiB     | ADR_PREL_LO21              |
| 275        | page hi21    | `0x3`/`0x7FFFF` | ±1 GiB pg  | ADR_PREL_PG_HI21           |

All of them wrap silently instead of erroring or falling back to an external relocation.

A secondary, related defect: `pc_offset` is **not** checked for 4-byte alignment before
`>> 2`, so a non-multiple-of-4 addend silently drops the low bits as well.

## Practical impact

* A `B.cond` whose target is more than ±1 MiB away (realistic in a large `.text`
  section), or a `TBZ`/`TBNZ` more than ±32 KiB away, miscompiles to a branch at a
  wildly different address with **no assembler diagnostic**.
* Because no external relocation is emitted, the linker cannot catch or fix it either —
  the object file simply contains a wrong instruction.
* Any caller that supplies a large `addend` triggers the same corruption.

## Suggested fix

Before masking, validate that `pc_offset` fits the field's *signed* range and is
word-aligned; if not, either:

1. return `Err(format!("branch out of range: ..."))` (matches GAS), **or**
2. fall back to emitting an external relocation (matching the existing undefined /
   cross-section code paths) so the linker can report `relocation truncated to fit`.

Approximate ranges for the check:

```text
imm26 (282/283):  -(1 << 27) <= pc_offset < (1 << 27)         // ±128 MiB
imm19 (280/273):   -(1 << 21) <= pc_offset < (1 << 21)         // ±1 MiB
imm14 (279):       -(1 << 15) <= pc_offset < (1 << 15)         // ±32 KiB
adr21  (274):      -(1 << 20) <= pc_offset < (1 << 20)         // ±1 MiB
```

and `assert!(pc_offset % 4 == 0)` (or fold the misalignment into a diagnostic).

## Test artifacts

* Failing property: `prop_branch_out_of_range_must_not_silently_truncate`
  (`src/backend/arm/assembler/elf_writer.rs`, in `mod tests`).
* Minimal failing input recorded by proptest:
  `proptest-regressions/backend/arm/assembler/elf_writer.txt`.

The four sibling properties all pass and serve as the positive reference / contract
suite for this function:

* `prop_branch_undefined_symbol_emits_external_reloc` ✓
* `prop_branch_cross_section_uses_section_symbol` ✓
* `prop_branch_jump26_call26_round_trips` ✓ (round-trip oracle on imm26)
* `prop_branch_condbr19_tstbr14_round_trips` ✓ (round-trip oracle on imm19/imm14)
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/121
