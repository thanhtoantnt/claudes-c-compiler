# Bug Report: `encode_neon_sri` panics on oversized / negative shift (debug overflow)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sri`
**Severity:** Medium

## Summary

`encode_neon_sri` derives the `immh:immb` field with a plain subtraction
`2*esize - shift`. When the shift exceeds `2*esize`, or when a **negative**
immediate is supplied (`get_imm` returns an `i64`, which is cast to `u32` and
becomes `u32::MAX`), the subtraction underflows. Under `debug_assertions` (the
default for `cargo test`) this panics with `attempt to subtract with overflow`,
aborting the host process instead of returning `Err`. In a release build the
value wraps to a bogus (often UNDEFINED) word.

This is distinct from the silent-misencode sibling
(`encode_neon_sri_silent_out_of_range_shift.md`): that report covers
panic-free out-of-range shifts in `[0, 2*esize]`; this one covers the
panic-inducing inputs `shift > 2*esize` and any negative immediate.

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;   // negative i64 -> u32::MAX
...
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,     // 16u32 - shift underflows when shift > 16
    "4h" | "8h"  => (32 - shift) & 0x1F,
    "2s" | "4s"  => (64 - shift) & 0x3F,
    "2d"         => (128 - shift) & 0x7F,
    ...
};
```

`16u32 - shift` is an unsigned subtraction; for `shift = 17` (or `shift =
u32::MAX` from a negative `i64`) it underflows. The trailing `& 0xF` mask does
not prevent the underflow panic — it is applied after the panic-triggering
operation. There is no input validation before the subtraction.

## Reproduction

```
Input:  sri v0.8b, v1.8b, #17        (esize = 8; 16u32 - 17 underflows)
Input:  sri v0.8b, v1.8b, #-1        (negative -> u32::MAX; 16u32 - u32::MAX underflows)
Actual: thread panicked at 'attempt to subtract with overflow'
Expected: Err
```

Witnessed by `catch_unwind` in the test (debug build):

```
$ cargo test --lib neon_sri_pbt -- --ignored sri_panics_on_overflow_shift
test sri_panics_on_overflow_shift ... ok        // "ok" = the panic was caught/documented
```

The remaining cases `(0,1,"8b",17)`, `(0,1,"4h",33)`, `(0,1,"4s",65)`,
`(0,1,"8b",-1)`, `(0,1,"2d",-7)` all panick instead of returning `Err`.

## Impact

A malicious or merely buggy caller can pass an oversized or negative shift and
crash the assembler process. Even without an attacker, any code path that
forwards an unvalidated `i64` (the natural type from `get_imm`) into `SRI` will
abort the host. The negative-immediate path is reachable through normal parsing
(e.g. a constant-folded `-1`), so this is not purely theoretical.

## Suggested Fix

Validate sign and range before the subtraction (also fixes the sibling
silent-misencode report):

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 1 {
    return Err(format!("sri: shift must be in [1, esize], got {}", shift_i));
}
let shift = shift_i as u32;
let (esize, immh_immb) = match arr_d.as_str() {
    "8b" | "16b" => (8u32,  16u32 - shift),
    "4h" | "8h"  => (16u32, 32u32 - shift),
    "2s" | "4s"  => (32u32, 64u32 - shift),
    "2d"         => (64u32, 128u32 - shift),
    _ => return Err(format!("unsupported sri arrangement: {}", arr_d)),
};
if shift > esize {
    return Err(format!("sri: shift {} out of range [1, {}]", shift, esize));
}
```

## Regression Property

Failing property: `sri_panics_on_overflow_shift` (documents the panic via
`catch_unwind`; passes today precisely *because* the encoder panics instead of
returning `Err`).

```rust
#[test]
#[ignore]
fn sri_panics_on_overflow_shift() {
    let cases: &[(u32, u32, &str, i64)] = &[
        (0, 1, "8b", 17),  // esize=8: 16 - 17 underflows
        (0, 1, "4h", 33),  // esize=16: 32 - 33 underflows
        (0, 1, "4s", 65),  // esize=32: 64 - 65 underflows
        (0, 1, "8b", -1),  // negative -> u32::MAX -> 16 - u32::MAX underflows
        (0, 1, "2d", -7),  // negative -> 128 - u32::MAX underflows
    ];
    for &(rd, rn, arr, shift) in cases {
        let ops = vec![
            Operand::RegArrangement { reg: format!("v{}", rd), arrangement: arr.to_string() },
            Operand::RegArrangement { reg: format!("v{}", rn), arrangement: arr.to_string() },
            Operand::Imm(shift),
        ];
        let result = catch_unwind(AssertUnwindSafe(|| encode_neon_sri(&ops)));
        assert!(result.is_err(),
            "expected a panic for sri {} shift {} (debug overflow); it should return Err",
            arr, shift);
    }
}
```

Reproduce: `cargo test --lib neon_sri_pbt -- --ignored sri_panics_on_overflow_shift`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/241
