# Bug Report: `encode_ldrsw` silently truncates out-of-range memory offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldrsw`
**Severity:** High

## Summary

The `[base, #offset]` (unsigned-offset) form of `encode_ldrsw` first tries the unsigned-offset encoding, and on failure falls through to the **unscaled** (LDURSW) encoding. The unscaled branch unconditionally masks the offset to 9 bits via `(*offset as i32) & 0x1FF` **without checking that the offset fits in the 9-bit signed `imm9` field (range [-256, 255])**.

## Encodable range (ARMv8-A ARM, §C4.1.65)

The `[base, #imm]` operand form is encodable by **exactly one** of:
- **LDRSW (unsigned offset):** `imm12 * 4`, `imm12 ∈ [0, 4095]` → `imm ∈ {0, 4, …, 16380}`, 4-aligned
- **LDURSW (unscaled):** signed `imm9 ∈ [-256, 255]`

So the encodable union is `[-256, 255] ∪ {multiples of 4 in [0, 16380]}`. Anything outside this union is **not representable** and the ARM ARM mandates the assembler reject it.

## Root Cause

```rust
// Unscaled: LDURSW
let imm9 = (*offset as i32) & 0x1FF;                       // ← NO RANGE CHECK
let word = (((0b10 << 30) | (0b111 << 27)) | (0b10 << 22))
        | ((imm9 as u32 & 0x1FF) << 12)) | (rn << 5) | rt;
return Ok(EncodeResult::Word(word));
```

## Reproduction

**Input:** offset = 16384 (not 4-aligned, > 16380)

**Expected:** `Err` — offset out of range; must be multiple of 4 in [0, 16380]

**Actual:** `Ok(Word(0xB8800000))` — encodes as `ldursw x0, [x1, #0]` (offset silently truncated)

**Minimal failing input:** `ldrsw x0, [x1, #16384]`

## Impact

Wrong load/store addresses are emitted into object code with no assembler error. This produces silently incorrect binaries (loads from the wrong address). The same defect affects `encode_ldr_str`, `encode_ldrs`, `encode_ldp_stp`, `encode_ldur_stur`.

## Suggested Fix

Validate the offset before masking:

```rust
let imm9_val = *offset as i32;
if !(-256..=255).contains(&imm9_val) {
    return Err(format!(
        "ldrsw: offset {} out of range; must be a multiple of 4 in [0, 16380] (unsigned) or in [-256, 255] (unscaled)",
        offset
    ));
}
let imm9 = (imm9_val as u32) & 0x1FF;
```

## Regression Property

Failing property: `prop_out_of_range_mem_offset_rejected`

```rust
prop_assert!(encode_ldrsw(&[xreg(0), mem_offset(xreg(1), 16384)], true).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/116