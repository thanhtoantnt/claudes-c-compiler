# Bug Report: `encode_neon_tbl` panics on an empty table register list

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbl`
**Severity:** Medium

## Summary

`encode_neon_tbl` indexes the table-register vector unconditionally with `&regs[0]` before checking whether the list is non-empty. When the second operand is an **empty** `Operand::RegList(vec![])`, this panics with index-out-of-bounds instead of returning `Err`, violating the error contract every other malformed-input path in the function honors.

## Root Cause

```rust
let (rn, num_regs) = match &operands[1] {
    Operand::RegList(regs) => {
        let first_reg = match &regs[0] {  // <-- BUG: no emptiness check
            Operand::RegArrangement { reg, .. } => parse_reg_num(reg).ok_or("invalid reg")?,
            Operand::Reg(name) => parse_reg_num(name).ok_or("invalid reg")?,
            _ => return Err("tbl: expected register list as second operand".to_string()),
        };
        (first_reg, regs.len() as u32)
    }
    _ => return Err("tbl: expected register list as second operand".to_string()),
};
```

`regs` is never guarded with `is_empty()`.

## Reproduction

**Input:** `tbl v0.8b, {}, v0.8b, {}` (empty register list)

**Expected:** `Err` — table register list must be non-empty

**Actual:** **Panic** with "index out of bounds"

## Impact

Violates error contract. Other malformed-input paths return `Err`, but empty list panics. Affects production reliability.

## Suggested Fix

Guard the empty list:

```rust
Operand::RegList(regs) => {
    if regs.is_empty() {
        return Err("tbl: table register list must be non-empty".to_string());
    }
    // (optional) if regs.len() > 4 { return Err(...); }
    let first_reg = match &regs[0] { /* ... unchanged ... */ };
    (first_reg, regs.len() as u32)
}
```

## Regression Property

Failing property: `prop_empty_list_does_not_panic`

```rust
prop_assert!(encode_neon_tbl(&[xreg(0), xreg(0), Operand::RegList(vec![])]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/84