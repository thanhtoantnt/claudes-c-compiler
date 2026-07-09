# Bug Report: `encode_cas` silently drops non-zero memory offset (`[Xn, #imm]`)

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_cas`
**Severity:** High

## Summary

`encode_cas` matches memory operand with `Operand::Mem { base, .. }`, ignoring `offset`. ARMv8.1-A CAS supports **only** `[Xn|SP]` addressing — no immediate-offset, pre-index, post-index, or register-offset forms. `cas x0, x1, [x2, #16]` silently encodes as `cas x0, x1, [x2]`.

## Root Cause

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("cas: invalid base")?,
    _ => return Err("cas requires memory operand [Xn]".to_string()),
};
```

The `..` pattern discards `offset`. Same defect in `encode_swp`.

## Reproduction

**Input:** `cas x0, x1, [x2, #16]`

**Expected:** `Err` — cas requires [Xn] addressing with no offset (got offset 16)

**Actual:** `Ok(Word(0xC8A07C42))` — identical to `cas x0, x1, [x2]` (offset silently dropped)

**Minimal failing input:** offset = 1

## Impact

Silent miscompilation: operates on wrong address (off by N bytes). Atomic compare-and-swap data-correctness hazard with no build-time signal. Other memory forms (pre/post-index, register-offset) correctly rejected.

## Suggested Fix

Explicitly reject non-zero offset:

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, offset: 0 }) => {
        parse_reg_num(base).ok_or("cas: invalid base")?
    }
    Some(Operand::Mem { base, offset }) => {
        return Err(format!(
            "{} requires [Xn] addressing with no offset (got offset {})",
            mnemonic, offset
        ));
    }
    _ => return Err("cas requires memory operand [Xn]".to_string()),
};
```

## Regression Property

Failing property: `prop_nonzero_immediate_offset_rejected`

```rust
prop_assert!(encode_cas("cas", &[xreg(0), xreg(1), mem_offset(xreg(2), 16)]).is_err());
```

## PBT Results (module `prop_encode_cas_offset_tests`)

| Property | Result |
|---|---|
| `prop_zero_offset_accepted_and_invariant` | PASS |
| `prop_nonzero_immediate_offset_rejected` | **FAIL** |
| `prop_pre_index_writeback_rejected` | PASS |
| `prop_post_index_writeback_rejected` | PASS |
| `prop_register_offset_rejected` | PASS |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/166