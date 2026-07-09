# Bug Report: `encode_ldaxr_stlxr` silently drops non-zero `[Xn, #imm]` offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldaxr_stlxr`
**Severity:** High

## Summary

Per the ARMv8-A Architecture Reference Manual (§C4.1.49 LDAXR, §C4.1.116 STLXR), the exclusive load/store register instructions address memory using **only `[Xn]`** — there is no immediate-offset, pre-index, or post-index encoding. The encoder pattern-matches the memory operand with `..` and **silently discards the offset**, emitting the `[Xn]` (offset 0) word.

The result is a *syntactically valid* but *semantically wrong* instruction word returned as `Ok`, so the assembler produces a program that accesses `[Xn]` instead of the programmer's intended address, with no diagnostic.

## Root Cause

```rust
let rn = match operands.get(1) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,  // offset dropped
    _ => return Err("ldaxr needs memory operand".to_string()),
};
```

The `..` pattern discards the offset field. The store branch (`operands.get(2)`) has the identical bug.

## Reproduction

**Input:** `stlxr w0, x1, [x2, #1]`

**Expected:** `Err` — only [Xn] addressing is supported, offset 1 is invalid

**Actual:** `Ok(Word(0xC800FC41))` — exactly `stlxr w0, x1, [x2]` (#1 became #0)

**Minimal failing input:** off = 1, is_load = false

## Impact

Silent mis-compilation produces wrong machine code. Programmers write `[Xn, #imm]` but get `[Xn]`. The assembler emits valid-looking instructions that access wrong memory locations, with no diagnostic. This can lead to silent data corruption or crashes.

## Suggested Fix

Validate the offset before encoding:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "ldaxr/stlxr: only [Xn] addressing is supported, offset {} is invalid",
            offset
        ));
    }
    parse_reg_num(base).ok_or("invalid base")?
}
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected`

```rust
prop_assert!(encode_ldaxr_stlxr(&[wreg(0), xreg(1), mem_offset(xreg(2), 1)], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/204