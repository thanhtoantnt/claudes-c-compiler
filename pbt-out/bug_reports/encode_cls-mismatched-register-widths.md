# Bug Report: `encode_cls` silently accepts mismatched register widths

**Target:** `src/backend/arm/assembler/encoder/bitfield.rs` → `encode_cls`
**Severity:** Medium

## Summary

`encode_cls` derives `sf` bit **only from destination `Rd`**, discarding source `Rn` width. Mismatched operands like `CLS X0, W0` silently encoded using `Rd` width.

## Root Cause

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, _) = get_reg(operands, 1)?;   // <-- Rn width discarded
let sf = sf_bit(is_64);
```

## Reproduction

**Input:** `cls x0, w0`

**Expected:** `Err` — CLS requires source and destination registers of the same size

**Actual:** `Ok(Word(0xDAC01400))` — encodes as `CLS X0, X0`

**Minimal failing input:** d = 0, n = 0

**Other failing input:** `cls w0, x0` → `Ok(0x5AC01400)` (`CLS W0, W0`)

## Impact

Silent mis-encoding: mixed-width operations accepted, producing semantically wrong instructions. Same defect class in `encode_clz`, `encode_rbit`, `encode_rev`, `encode_rev16`, `encode_rev32`.

## Suggested Fix

Validate width coherence:

```rust
let (rd, is_64) = get_reg(operands, 0)?;
let (rn, src_is_64) = get_reg(operands, 1)?;
if is_64 != src_is_64 {
    return Err("CLS requires source and destination registers of the same size".into());
}
```

## Regression Property

Failing property: `prop_rejects_mixed_width_operands`

```rust
prop_assert!(encode_cls(&[xreg(0), wreg(0)]).is_err());  // X, W
prop_assert!(encode_cls(&[wreg(0), xreg(0)]).is_err());  // W, X
```

## PBT Results (module `prop_encode_cls_tests`)

| Property | Result |
|---|---|
| `prop_cls_field_placement` | PASS |
| `prop_cls_known_constants` | PASS |
| `prop_cls_xor_clz_is_only_bit_10` | PASS |
| `prop_width_changes_only_sf` | PASS |
| `prop_rejects_malformed_operands` | PASS |
| `prop_rejects_mixed_width_operands` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/140