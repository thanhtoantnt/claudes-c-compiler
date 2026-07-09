# Bug Report: `encode_csetm` accepts reserved `AL`/`NV` conditions and emits an undefined encoding

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csetm`
**Severity:** Medium

## Summary

`CSETM <Rd>, <cond>` is an alias of `CSINV <Rd>, <XZR>, <XZR>, invert(<cond>)`. Per the ARM ARM (C6.2.44) the aliased CSINV condition must **not** be `AL` (`1110`) or `NV` (`1111`) — those are reserved condition-field values for the conditional-select group. Because `invert` swaps `AL <-> NV`, a user-facing `csetm Rd, al` or `csetm Rd, nv` produces a CSINV whose condition field is `NV` / `AL`: a reserved encoding. Reference assemblers reject these with an error.

## Root Cause

```rust
fn encode_cond(cond: &str) -> u32 {
    let cond4 = match cond.to_lowercase().as_str() {
        "eq" => 0b0000, "ne" => 0b0001, "cs" => 0b0010, ...
        "al" => 0b1110, "nv" => 0b1111, ...
    };
    cond4
}
```

No validation that `cond4 != 0b1110 && cond4 != 0b1111`.

## Reproduction

**Input:** `csetm x0, al`

**Expected:** `Err` — condition codes AL and NV are invalid for this instruction

**Actual:** `Ok(EncodeResult::Word(0xDA9FF3E0))` — emits reserved encoding with cond field = NV

**Minimal failing input:** case = 0 (csetm x0, al)

## Impact

Emits an architecturally-rejected / disassembler-undefined word with no diagnostic. The condition field contains reserved values that hardware may treat as undefined behavior.

## Suggested Fix

Add validation before encoding:

```rust
if cond4 == 0b1110 || cond4 == 0b1111 {
    return Err(format!("invalid condition: {} is reserved", cond));
}
```

## Regression Property

Failing property: `prop_rejects_al_nv_conditions`

```rust
prop_assert!(encode_csetm(&[xreg(0), Operand::Cond("al".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/31