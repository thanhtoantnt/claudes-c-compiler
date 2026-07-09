# Bug Report: `encode_ldr_str` silently truncates out-of-range immediates

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldr_str`
**Severity:** High

## Summary

`encode_ldr_str` never validates that the immediate offset fits in the encoding field it actually uses. When an offset is too large for the unsigned-offset form, it falls through to the LDUR/STUR (unscaled) form and masks the offset into a 9-bit field with `& 0x1FF`, producing a syntactically valid instruction whose address displacement is **unrelated to the source operand**. No `Err` is returned.

For the `[base, #imm]` form, an offset is representable iff it lies in the union `[-256, 32760]`. Anything strictly outside this MUST be rejected.

## Root Cause

In the `Operand::Mem` arm, after the unsigned-offset branch fails the range check, control falls into:

```rust
// Unscaled offset (LDUR/STUR form)
let imm9 = (*offset as i32) & 0x1FF;          // <-- silent truncation, no bounds check
...
return Ok(EncodeResult::Word(word));          // <-- returns Ok on garbage
```

Same issue affects `Operand::MemPreIndex` and `Operand::MemPostIndex`.

## Reproduction

**Input:** `ldr x0, [x1, #32761]`

**Expected:** `Err` — offset out of range [-256, 32760]

**Actual:** `Ok(Word(0xF85F9020))` — encoded as `ldur x0, [x1, #-7]` (wrong address)

**Minimal failing input:** offset = 32761, negative = false

## Impact

Silent mis-compilation with wrong address displacement. For example, `ldr x0, [x1, #32761]` encodes as `ldur x0, [x1, #-7]` — a completely different address. The assembler produces valid-looking instructions that target wrong memory locations, with no diagnostic. This can lead to silent data corruption or crashes.

## Suggested Fix

Add a range check before each `imm9`/`imm12` packing:

```rust
let imm9 = *offset as i32;
if !(-256..=255).contains(&imm9) {
    return Err(format!("ldr/str offset out of range [-256, 32760]: {}", offset));
}
let imm9 = imm9 as u32 & 0x1FF;
```

Apply analogous `[-256, 255]` guards in the pre/post-index arms.

## Regression Property

Failing property: `prop_out_of_range_offset_is_rejected`

```rust
prop_assert!(encode_ldr_str(&[xreg(0), mem_offset(xreg(1), 32761)], true, 3, false, false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/48