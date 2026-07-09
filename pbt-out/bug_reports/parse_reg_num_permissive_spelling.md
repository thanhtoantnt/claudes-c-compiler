# Bug Report: `parse_reg_num` over-accepts malformed register names

**Target:** `src/backend/arm/assembler/encoder/mod.rs` → `parse_reg_num`
**Severity:** Low

## Summary

`parse_reg_num` parses the numeric suffix of a register name with `name[1..].parse::<u32>()`. Rust's integer `FromStr` accepts an optional leading `+` and arbitrary leading zeros, so several inputs that are **not** valid AArch64 register spellings are silently accepted and mapped to a real register number instead of being rejected (`None`).

## Root Cause

```rust
let num: u32 = name[1..].parse().ok()?;
if num <= 31 { Some(num) } else { None }
```

The `parse()` call accepts malformed input like `"x+5"` or `"x007"` without validation.

## Reproduction

**Input:** `parse_reg_num("x+5")`

**Expected:** `None` — `"x+5"` is not a valid register

**Actual:** `Some(5)` — incorrectly accepted

**Other malformed inputs:**
- `parse_reg_num("x+0")` → `Some(0)` — expected `None`
- `parse_reg_num("x007")` → `Some(7)` — expected `None`
- `parse_reg_num("w+31")` → `Some(31)` — expected `None`

## Impact

**Conceptual risk**: The documented contract is "Parse a register name to its 5-bit encoding number (0–30, 31 for sp/zr)". The current implementation does not enforce that the suffix is a *canonical decimal integer*, so a hand-written or attacker-controlled `.s` input could name a register in a non-standard way and still assemble. This weakens the negative contract, though there is **no silent truncation** — the value range (`<= 31`) is correctly enforced.

The built-in assembler is normally fed by codegen that never emits `x+5`/`x007`, so in practice no mis-encoding occurs today.

## Suggested Fix

Reject any suffix that is not a plain run of ASCII digits with no sign and no redundant leading zero:

```rust
let suffix = &name[prefix.len()..];
if !suffix.bytes().all(|b| b.is_ascii_digit())
    || (suffix.starts_with('0') && suffix.len() > 1)
{
    return None;
}
let num: u32 = suffix.parse().ok()?;
if num <= 31 { Some(num) } else { None }
```

## Regression Property

Failing property: `non_numeric_suffix_rejected`

```rust
prop_assert!(parse_reg_num("x+5").is_none());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/118