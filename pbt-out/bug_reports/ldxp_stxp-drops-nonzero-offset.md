# Bug Report: `encode_ldxp_stxp` silently drops non-zero immediate offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldxp_stxp`
**Severity:** High

## Summary

`encode_ldxp_stxp` matches memory operand with `Operand::Mem { base, .. }` and **silently discards the offset field**. Per ARMv8-A, LDXP/STXP have **no immediate-offset form** — only `[Xn|SP]` addressing is permitted. An operand like `[x2, #8]` is unrepresentable and should be rejected. Instead, the encoder treats `ldxp x0, x1, [x2, #8]` identically to `ldxp x0, x1, [x2]` — wrong address, no diagnostic.

## Root Cause

```rust
let rn = match operands.get(2) {                 // load branch
    Some(Operand::Mem { base, .. }) => parse_reg_num(base)...,
    _ => return Err(...),
};
```

The `..` pattern discards `offset`. Store branch at `operands.get(3)` has identical bug.

## Reproduction

**Input:** `ldxp x0, x1, [x2, #8]`

**Expected:** `Err` — ldxp does not support immediate offset (got #8); use [Rn] only

**Actual:** `Ok(Word(...))` — encodes as `ldxp x0, x1, [x2]` (offset silently dropped)

**Minimal failing input:** is_load=false, acquire_release=false, rt_num=0, rt2_num=0, base_num=0, ws_num=0, off=1

## Impact

Incorrect code generation for any source writing non-zero offset on exclusive pair instruction, with no assembler diagnostic. Same defect class as `encode_ldxr_stxr`.

## Suggested Fix

Inspect and reject non-zero offset in both branches:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "ldxp/ldaxp does not support an immediate offset (got #{}); use [Rn] only",
            offset
        ));
    }
    parse_reg_num(base).ok_or("ldxp needs memory operand")?
}
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected`

```rust
prop_assert!(encode_ldxp_stxp(&[xreg(0), xreg(1), mem_offset(xreg(2), 1)], true, false).is_err());
```

## PBT Results (module `prop_encode_ldxp_stxp_offset_tests`)

| Property | Result |
|---|---|
| `prop_offset_does_not_affect_word` | PASS |
| `prop_nonzero_offset_rejected` | **FAIL** |
| `prop_zero_offset_accepted` | PASS |
| `prop_common_offsets_rejected` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/179