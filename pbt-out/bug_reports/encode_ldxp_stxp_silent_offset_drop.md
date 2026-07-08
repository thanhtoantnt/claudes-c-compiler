# Bug Report: `encode_ldxp_stxp` silently drops non-zero memory offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldxp_stxp`

## Summary

LDXP/STXP/LDAXP/STLXP pair-exclusive instructions support only `[Xn]` addressing. `encode_ldxp_stxp` matches `Operand::Mem { base, .. }` and ignores the offset field, so `[Xn,#imm]` is accepted and encoded as `[Xn]`.

## Reproduction

Direct probe from the PBT campaign:

```text
ldxp x0, x1, [x2, #8]
```

Actual result:

```text
Ok(Word(0xC87F0440))
```

That is the same word as `ldxp x0, x1, [x2]`.

## Impact

Silent miscompilation: invalid source with a non-zero address offset assembles to an offset-zero pair-exclusive instruction.

## Suggested fix

Reject non-zero offsets in the `Operand::Mem` arm:

```rust
match operands.get(mem_index) {
    Some(Operand::Mem { base, offset }) if *offset == 0 => { /* encode */ }
    Some(Operand::Mem { offset, .. }) => return Err(format!("ldxp/stxp offset must be zero: {}", offset)),
    _ => return Err("ldxp/stxp needs memory operand".to_string()),
}
```

## Regression property

Failing property: `ldxp_stxp_nonzero_offset_rejected`

```rust
prop_assert!(encode_ldxp_stxp(&[xreg(0), xreg(1), mem_offset(xreg(2), 8), xreg(3)], true).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/51
