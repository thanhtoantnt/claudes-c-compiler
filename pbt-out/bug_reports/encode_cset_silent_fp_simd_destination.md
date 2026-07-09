# Bug Report: `encode_cset` silently accepts FP/SIMD destination registers

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_cset`
**Severity:** Medium

## Summary

`encode_cset` uses shared generic register parser accepting FP/SIMD names (`d`, `s`, `q`, `v`, `h`, `b`) and returning only numeric index. CSET is GP-register instruction; FP/SIMD destinations must be rejected.

## Root Cause

Shared `get_reg` accepts any register prefix, no bank validation. Destination numeric index reinterpreted as GP register.

## Reproduction

**Input:** `cset d0, eq`

**Expected:** `Err` — expected integer register, got d0

**Actual:** `Ok(Word(...))` — FP register silently reinterpreted as GP

**Minimal failing input:** dest = "d0" (or any `s`/`q`/`v`/`h`/`b`)

## Impact

Invalid FP/SIMD source accepted, assembled into GP-register instruction with same numeric index. Programmer's instruction silently corrupted.

## Suggested Fix

Use GP-only register parser for conditional-select alias family:

```rust
fn get_gp_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    let name = match &operands[idx] {
        Operand::Reg(r) => r.to_lowercase(),
        _ => return Err("expected register".to_string()),
    };
    if !matches!(name.chars().next(), Some('w') | Some('x')) {
        return Err(format!("expected integer register, got {}", name));
    }
    get_reg(operands, idx)
}
```

## Regression Property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_cset(&[dreg(0), cond("eq")]).is_err());  // FP dest
prop_assert!(encode_cset(&[sreg(0), cond("eq")]).is_err());
prop_assert!(encode_cset(&[vreg_arr(0, "8b"), cond("eq")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/30