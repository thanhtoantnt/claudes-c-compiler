# Bug Report: `encode_neon_pmull` does not validate operand arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_pmull`
**Severity:** High

## Summary

`encode_neon_pmull` silently accepts invalid arrangements. ARMv8-A defines only: `PMULL Vd.1Q, Vn.1D, Vm.1D` (Q=0) and `PMULL2 Vd.1Q, Vn.2D, Vm.2D` (Q=1). Every other arrangement is UNALLOCATED.

## Root Cause

```rust
let (rd, _) = get_neon_reg(operands, 0)?;   // arrangement discarded
let (rn, _) = get_neon_reg(operands, 1)?;   // arrangement discarded
let (rm, _) = get_neon_reg(operands, 2)?;   // arrangement discarded
let q = if is_pmull2 { 1u32 } else { 0 };  // from is_pmull2 flag only
```

## Reproduction

**Input:** `pmull v0.8b, v1.8b, v2.8b`

**Expected:** `Err` — PMULL requires 1D source arrangements

**Actual:** `Ok(Word(0x0EE2E020))` — identical to canonical form (arrangements silently discarded)

**Minimal failing input:** arr = "8b" for PMULL (any arrangement != "1d")

## Impact

Invalid arrangements silently accepted, producing wrong code. LLVM rejects these with "invalid operand for instruction".

## Suggested Fix

Validate arrangements:

```rust
match (arr_d.as_str(), arr_n.as_str(), is_pmull2) {
    ("1q", "1d", false) => { /* OK */ }
    ("1q", "2d", true) => { /* OK */ }
    _ => return Err(format!("PMULL requires: Vd.1Q, Vn.1D, Vm.1D or Vd.1Q, Vn.2D, Vm.2D")),
}
```

## Regression Property

Failing property: `pmull_rejects_invalid_arrangements`

```rust
prop_assert!(encode_neon_pmull(&[neon_reg(0, "8b"), neon_reg(1, "8b"), neon_reg(2, "8b")], false).is_err());
prop_assert!(encode_neon_pmull(&[neon_reg(0, "2s"), neon_reg(1, "2s"), neon_reg(2, "2s")], false).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/187