# Bug Report: `resolve_local_branches` silently truncates out-of-range branch offsets

**Target:** `src/backend/arm/assembler/elf_writer.rs` → `ElfWriter::resolve_local_branches`
**Severity:** High

## Summary

When resolving a same-section branch relocation, `resolve_local_branches` patches the instruction's immediate field by **masking** the PC-relative offset into the available bit width, with **no range validation**. If the offset exceeds the instruction's encodable range, the high bits are silently dropped (the value wraps), producing an instruction that branches to a completely different target than requested. The function returns `Ok(())` and emits no external relocation, so the corruption is invisible.

This violates the AArch64 ELF ABI range contract and the behavior of GAS, which errors at assembly time.

## Root Cause

```rust
let pc_offset = (target_offset as i64) - (reloc.offset as i64) + reloc.addend;
...
280 => { // R_AARCH64_CONDBR19
    let imm19 = ((pc_offset >> 2) as u32) & 0x7FFFF;   // silent wrapping
    word |= imm19 << 5;
}
```

The masking-without-validation pattern is identical for every relocation type:
- B/BL (imm26): ±128 MiB, masked with `0x3FFFFFF`
- CONDBR19 (imm19): ±1 MiB, masked with `0x7FFFF`
- TBZ/TBNZ (imm14): ±32 KiB, masked with `0x3FFF`
- ADR (imm21): ±1 MiB, masked with `0x3`/`0x7FFFF`

## Reproduction

**Input:** CONDBR19 with offset 2,000,000 bytes (≈1.9 MiB, out of range)

**Expected:** `Err` — branch out of range

**Actual:** Encodes branch to -97,152 bytes (wrong target), returns `Ok(())`

**Minimal failing input:** reloc_type = 280, reloc.offset = 4, target = ".text"+4, addend = 2_000_000

## Impact

A `B.cond` whose target is more than ±1 MiB away (realistic in large `.text` sections) miscompiles to a wildly different address with **no assembler diagnostic**. Because no external relocation is emitted, the linker cannot catch or fix it. Any large `addend` triggers the same corruption.

## Suggested Fix

Validate the offset before masking:

```rust
if pc_offset < -(1 << 21) || pc_offset >= (1 << 21) {
    return Err(format!("branch out of range: {}", pc_offset));
}
```

Apply appropriate ranges for each relocation type:
- imm26 (282/283): `-(1 << 27) <= pc_offset < (1 << 27)` (±128 MiB)
- imm19 (280/273): `-(1 << 21) <= pc_offset < (1 << 21)` (±1 MiB)
- imm14 (279): `-(1 << 15) <= pc_offset < (1 << 15)` (±32 KiB)
- adr21 (274): `-(1 << 20) <= pc_offset < (1 << 20)` (±1 MiB)

## Regression Property

Failing property: `prop_branch_out_of_range_must_not_silently_truncate`

```rust
prop_assert!(resolve_local_branches(&[Relocation { offset: 4, addend: 2_000_000, reloc_type: 280 }]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/121