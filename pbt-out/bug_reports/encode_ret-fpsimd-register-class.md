# Bug Report: `encode_ret` silently accepts FP/SIMD register class

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs` → `encode_ret`
**Severity:** Medium

## Summary

`parse_reg_num` accepts FP/SIMD register prefixes (`d`, `s`, `q`, `v`, `h`, `b`) as if they were GP registers. `RET` operand must be GP `<Xn>`. FP/SIMD register numbers silently reused as GP register numbers.

## Root Cause

```rust
// parse_reg_num accepts all prefixes including FP/SIMD
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```

No GP-only validation in `encode_ret`.

## Reproduction

**Input:** `ret d0`

**Expected:** `Err` — invalid operand for RET (expected GP register)

**Actual:** `Ok(Word(0xD65F0000))` — `ret d0` encodes as `ret x0`

**Other failing inputs:** `ret v5` → `ret x5`, `ret q31` → `ret xzr`

## Impact

FP/SIMD register numbers silently reused as GP registers. Typos or parser bugs produce wrong control flow.

## Suggested Fix

Validate GP register class:

```rust
let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => return Err(...) };
if !matches!(rn_name.chars().next(), Some('x') | Some('w')) {
    return Err(format!("RET requires GP register, got {}", rn_name));
}
```

## Regression Property

Failing property: `prop_rejects_fp_simd_register_class`

```rust
prop_assert!(encode_ret(&[Operand::Reg("d0".into())]).is_err());
prop_assert!(encode_ret(&[Operand::Reg("v5".into())]).is_err());
prop_assert!(encode_ret(&[Operand::Reg("q31".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/216