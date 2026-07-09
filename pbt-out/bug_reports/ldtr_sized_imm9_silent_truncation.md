# Bug Report: `encode_ldtr_sized` silently truncates out-of-range LDTR/STTR immediates

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldtr_sized`
**Severity:** High

## Summary

The LDTR/STTR `imm9` field is a **signed 9-bit** immediate with valid range `[-256, 255]` (ARMv8 ARM §C4.1.66). The encoder masks out-of-range offsets into the 9-bit field instead of rejecting them:

```rust
let imm9_enc = (imm9 as u32) & 0x1FF;
let word = (size << 30) | (0b111 << 27) | (opc << 22) | (imm9_enc << 12) | (0b10 << 10) | (rn << 5) | rt;
```

`& 0x1FF` silently wraps any offset: `#256` → `#0`, `#512` → `#0`, `#257` → `#1`, `#-257` → `#-1`. The assembler emits a correct-looking but wrong instruction word.

## Root Cause

No range validation before masking. The code simply `& 0x1FF` the offset.

## Reproduction

**Input:** `ldtr x0, [x1, #256]`

**Expected:** `Err` — imm9 offset out of range [-256, 255]

**Actual:** `Ok(Word(0xF8400820))` — encoded as `ldtr x0, [x1, #0]` (offset silently truncated)

**Minimal failing input:** excess = 1, negative = false → offset = 256

## Impact

Silent mis-compilation: wrong machine code emitted with no diagnostic. Wrong addresses are loaded/stored with no assembler error. Same silent-truncation pattern affects sibling functions in this file (`encode_ldr_str`, `encode_ldur_stur`, `encode_ldp_stp`, `encode_ldnp_stnp`).

## Suggested Fix

Validate the offset before masking:

```rust
if imm9 < -256 || imm9 > 255 {
    return Err(format!("ldtr/sttr: imm9 offset {} out of range [-256, 255]", imm9));
}
```

## Regression Property

Failing property: `prop_out_of_range_imm9_is_rejected`

```rust
prop_assert!(encode_ldtr_sized(&[xreg(0), mem_offset(xreg(1), 256)], true, 0).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/206