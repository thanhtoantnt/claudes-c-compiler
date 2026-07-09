# Bug Report: `encode_cset` accepts reserved `AL`/`NV` conditions and emits an undefined encoding

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cset`
**Severity:** Medium

## Summary

`CSET <Rd>, <cond>` is an alias of `CSINC <Rd>, <XZR>, <XZR>, invert(<cond>)`. Per the ARM ARM (C6.2.43) the aliased CSINC condition must **not** be `AL` (`1110`) or `NV` (`1111`) — those are reserved condition-field values for the conditional-select group. Because `invert` swaps `AL <-> NV`, a user-facing `cset Rd, al` or `cset Rd, nv` produces a CSINC whose condition field is `NV` / `AL`: a reserved encoding. Reference assemblers reject these.

## Root Cause

```rust
fn encode_cond(cond: &str) -> u32 {
    // cond_str -> cond4, but no validation for AL/NV
    let cond4 = match cond.to_lowercase().as_str() {
        "eq" => 0b0000, "ne" => 0b0001, "cs" => 0b0010, ...
        "al" => 0b1110, "nv" => 0b1111, ...
    };
    cond4
}
```

No check that `cond4 != 0b1110 && cond4 != 0b1111`.

## Reproduction

**Input:** `cset x0, al`

**Expected:** `Err` — condition codes AL and NV are invalid for this instruction

**Actual:** `Ok(EncodeResult::Word(0x9A9FF7E0))` — emits reserved encoding with cond field = NV

**Minimal failing input:** case = 0 (cset x0, al)

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
prop_assert!(encode_cset(&[xreg(0), Operand::Cond("al".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/29