# Bug Report: `encode_fmov` silently accepts width/precision-mismatched operands

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fmov`
**Severity:** Medium

## Summary

`encode_fmov` derives `sf`/`ftype` from **only one operand of each pair** and never validates width/precision coherence. GP↔FP pairs must match widths (`Dd↔Xn`, `Sd↔Wn`), and FP↔FP pairs must match precision. Mismatches produce wrong encodings.

## Root Cause

- GP→FP: `sf` from source GP register width (discarded)
- FP→GP: `sf` from dest GP register width (discarded)
- FP↔FP: `is_double = rd.starts_with('d') || rm.starts_with('d')` — OR not AND

## Reproduction

**Input:** `fmov d0, w1`

**Expected:** `Err` — FMOV D-register requires X-register GP source

**Actual:** `Ok(Word(0x9E670020))` — silently encodes as `fmov d0, x1` (W→X coerced)

**Other failing inputs:**
- `fmov s0, x1` → encodes as `fmov s0, w1` (X→W)
- `fmov d0, s1` → encodes as `fmov d0, d1` (S→D)

## Impact

Silent miscompilation: operand width/precision corrupted with no error. Wrong-width register access, architecturally UNALLOCATED encodings produced.

## Suggested Fix

Validate width/precision coherence before encoding:

```rust
// GP→FP: Dd ↔ Xn, Sd ↔ Wn
if rd_is_fp && !rm_is_fp {
    let expects_x = rd_name.starts_with('d');
    let has_x = rn_name.starts_with('x');
    if expects_x != has_x {
        return Err("FMOV: Dd requires Xn, Sd requires Wn".into());
    }
}
// FP↔FP: must share precision
if rd_is_fp && rm_is_fp {
    if rd_name.starts_with('d') != rm_name.starts_with('d') {
        return Err("FMOV: FP-to-FP move requires matching precision".into());
    }
}
```

## Regression Property

Failing property: `prop_fmov_rejects_mismatched_width_and_precision`

```rust
prop_assert!(encode_fmov(&[dreg(0), wreg(1)]).is_err());  // D-register needs X source
prop_assert!(encode_fmov(&[sreg(0), xreg(1)]).is_err());  // S-register needs W source
prop_assert!(encode_fmov(&[dreg(0), sreg(1)]).is_err());  // FP precision mismatch
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/145