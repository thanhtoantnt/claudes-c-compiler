# Bug Report: `encode_csetm` silently accepts FP/SIMD destination registers

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_csetm`
**Severity:** Medium

## Summary

`encode_csetm` uses shared generic register parser accepting FP/SIMD names (`d`, `s`, `q`, `v`, `h`, `b`) and returning only numeric index. CSETM is GP-register instruction; FP/SIMD destinations must be rejected.

## Root Cause

`parse_reg_num` treats every FP/SIMD prefix as valid, extracts only numeric index. No GP-bank validation:

```rust
fn parse_reg_num(name: &str) -> Option<u32> {
    // matches d/s/q/v/h/b prefixes and returns the trailing number
    ...
}
```

## Reproduction

**Input:** `csetm b0, eq`

**Expected:** `Err` — expected integer register, got b0

**Actual:** `Ok(Word(0x5A9F13E0))` — FP prefix silently discarded, index used as GP

**Minimal failing input:** prefix = "b", n = 0

## Impact

Invalid FP/SIMD destination accepted, assembled into GP instruction with same numeric index. Operates on wrong register file. Because `is_64bit_reg("b0") == false`, word silently forced to 32-bit (W) form — corrupts both register class and instruction width. Same gap affects conditional-select alias family.

## Suggested Fix

Use GP-only register parser:

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
prop_assert!(encode_csetm(&[breg(0), cond("eq")]).is_err());  // FP dest
prop_assert!(encode_csetm(&[dreg(0), cond("eq")]).is_err());
prop_assert!(encode_csetm(&[vreg_arr(0, "8b"), cond("eq")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/32