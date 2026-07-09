# Bug Report: `encode_ldaxr_stlxr` silently drops non-zero `[Xn, #imm]` offset

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldaxr_stlxr`
**Severity:** Medium

## Summary

`encode_ldaxr_stlxr` matches memory operand with `Operand::Mem { base, .. }`, **silently discarding the `offset` field**. Per ARMv8-A, exclusive load/store instructions support **only** `[Xn]` addressing — no immediate-offset, pre-index, or post-index form. An operand like `[x1, #8]` is unrepresentable and should be rejected. Instead the encoder emits offset-0 `[Xn]` encoding without warning.

## Root Cause

```rust
// load path (line 577)
Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
// store path (line 588)
Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
```

The `..` ignores `offset`. Same bug pattern in `encode_ldxr_stxr`, `encode_ldxp_stxp`, `encode_ldar_stlr`.

## Reproduction

**Input:** `stlxr w0, x1, [x2, #1]`

**Expected:** `Err` — ldaxr/stlxr supports only [Xn] addressing (no offset)

**Actual:** `Ok(Word(0xC801FC40))` — encodes as `stlxr w0, x1, [x2]` (offset silently dropped)

**Minimal failing input:** off = 1, is_load = false

## Impact

Silent acceptance of unrepresentable instruction. Emitted bytes are a *different* valid instruction than source requested — wrong code with no diagnostic. Worst case: load/store at wrong address in hand-written atomics.

## Suggested Fix

Reject any non-zero offset:

```rust
Some(Operand::Mem { base, offset }) if *offset == 0 => parse_reg_num(base).ok_or("invalid base")?,
_ => return Err("ldaxr/stlxr supports only [Xn] addressing (no offset)".to_string()),
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected`

```rust
prop_assert!(encode_stlxr(&[wreg(0), xreg(1), mem_offset(xreg(2), 1)]).is_err());
```

## PBT Results (module `prop_encode_ldaxr_stlxr_tests`)

| Property | Result |
|---|---|
| `prop_layout_vs_golden_and_l_bit` | PASS |
| `prop_fixed_bits_o0_and_reserved` | PASS |
| `prop_size_field_auto_and_forced` | PASS |
| `prop_malformed_operands_rejected` | PASS |
| `prop_nonzero_offset_rejected` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/157