# Bug Report — `encode_ldr_str` silently truncates out-of-range immediates

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` →
`pub(crate) fn encode_ldr_str(operands, is_load, size, is_signed, is_128bit)`

**Severity:** High (silent miscompilation — emits a valid-looking but *wrong*
instruction word instead of an assembler error).

**Found by:** property `prop_encode_ldr_str_tests::prop_out_of_range_offset_is_rejected`
(minimal failing input: `excess = 1, negative = false` → offset `32761`).

---

## Summary

`encode_ldr_str` never validates that the immediate offset fits in the encoding
field it actually uses. When an offset is too large for the unsigned-offset form,
it falls through to the LDUR/STUR (unscaled) form and masks the offset into a
9-bit field with `& 0x1FF`, producing a syntactically valid instruction whose
address displacement is **unrelated to the source operand**. No `Err` is returned.

## ARMv8-A spec (oracle)

For the `[base, #imm]` form, an offset is representable iff it lies in the union:

| Form           | Field  | Encodable range |
|----------------|--------|-----------------|
| unsigned off   | imm12  | `[0, 32760]` (imm12 × 8 for size=0b11) |
| unscaled (LDUR)| imm9   | `[-256, 255]` |

→ **union `[-256, 32760]`**. Anything strictly outside this MUST be rejected.

## Reproduction

```rust
// ldr x0, [x1, #32761]   — 32761 ∉ [-256, 32760], not 8-aligned, > imm9 max
let ops = vec![Operand::Reg("x0".into()), Operand::Mem { base: "x1".into(), offset: 32761 }];
encode_ldr_str(&ops, true, 0b11, false, false)
// ACTUAL:   Ok(Word(0xF85F9020))   // a *valid* LDUR x0,[x1,#-7]  (32761 & 0x1FF = 505 → signed −7)
// EXPECTED: Err(...)                 // offset unrepresentable
```

The encoded word decodes to `ldur x0, [x1, #-7]` — a completely different
address from the programmer's `#32761`. The same silent wrap occurs for any
out-of-range negative offset (e.g. `-1000 & 0x1FF = 24` → encoded as `#+24`).

## Root cause

In the `Operand::Mem` arm, after the unsigned-offset branch fails the range
check, control falls into:

```rust
// Unscaled offset (LDUR/STUR form)
let imm9 = (*offset as i32) & 0x1FF;          // <-- silent truncation, no bounds check
...
return Ok(EncodeResult::Word(word));          // <-- returns Ok on garbage
```

## Affected locations (all in `encode_ldr_str`)

1. **`Operand::Mem` LDUR fallthrough** — masks any out-of-range offset to 9 bits and returns `Ok`. *(the failing property)*
2. **`Operand::MemPreIndex`** — `let imm9 = (*offset as i32) & 0x1FF;` with no `[-256, 255]` check; e.g. `ldr x0,[x1,#1000]!` silently encodes writeback of `−24`.
3. **`Operand::MemPostIndex`** — same as pre-index.

## Suggested fix

Add a range check before each `imm9`/`imm12` packing and return `Err` on
overflow, e.g. in the `Mem` fallthrough:

```rust
let imm9 = *offset as i32;
if !(-256..=255).contains(&imm9) {
    return Err(format!("ldr/str offset out of range [-256, 32760]: {}", offset));
}
let imm9 = imm9 as u32 & 0x1FF;
```

and analogous `[-256, 255]` guards in the pre/post-index arms.

## Secondary finding (consistency, not a hard crash)

The `is_signed` parameter is honored in the `Operand::Mem` arm
(`opc = 0b10` when `is_signed`) but **silently ignored** in the
`MemPreIndex` and `MemPostIndex` arms, where `opc` is unconditionally
`0b01`/`0b00`. Callers passing `is_signed = true` with a writeback operand get
an unsigned opc — an inconsistent contract for a low-level API that takes
`is_signed` as an explicit argument.

## Properties

| # | Property | Status |
|---|----------|--------|
| 1 | `prop_unsigned_offset_layout` (ARM-ARM golden field placement) | ✅ pass |
| 2 | `prop_load_xor_store_is_opc_bit22` (load^store == 0x0040_0000) | ✅ pass |
| 3 | `prop_size_param_in_top_two_bits` (size → bits[31:30]) | ✅ pass |
| 4 | `prop_pre_post_index_layout` (golden pre/post encodings) | ✅ pass |
| 5 | `prop_out_of_range_offset_is_rejected` (negative contract) | ❌ **FAIL** |

Oracle: reference/field-placement against hand-derived ARMv8-A golden encodings
(§C4.1.64, §C4.1.66). Differential validation against `llvm-mc`/`aarch64-as`
was not possible (neither is installed in this environment).
