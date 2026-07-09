# Bug Report: `encode_neon_sli` panics on negative shift immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sli`
**Severity:** Medium

## Summary

`encode_neon_sli` (the AArch64 `SLI` — Shift Left and Insert — encoder) performs
**no range validation** on the `#shift` immediate, then casts it via
`get_imm(...)? as u32`. A negative immediate (`i64`) wraps to a huge `u32`, and
the subsequent `esize + shift` addition **overflows** `u32` in debug builds,
aborting the assembler with an `attempt to add with overflow` panic instead of
returning `Err`. This is the panic-on-negative facet of the defect (the
silent-acceptance facet is filed separately).

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;          // neon.rs:1264 — wraps negative i64
...
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (8 + shift) & 0xF,             // neon.rs:1271 — u32 overflow → panic
    "4h" | "8h"  => (16 + shift) & 0x1F,
    "2s" | "4s"  => (32 + shift) & 0x3F,
    "2d"         => (64 + shift) & 0x7F,
    _ => return Err(format!("unsupported sli arrangement: {}", arr_d)),
};
```

`get_imm` returns an `i64`. `sli v0.8b, v1.8b, #-1` gives `shift = -1`, which the
`as u32` cast wraps to `4294967295`; then `8u32 + 4294967295u32` overflows in
debug builds and panics at `neon.rs:1271`. A malformed user input should produce
`Err`, not abort the process.

## Reproduction

Property `prop_sli_rejects_out_of_range_shift`, run via
`cargo test --lib neon_sli_pbt -- --ignored`, panics on the negative path:

- **Input:** `sli v0.8b, v1.8b, #-1` (any `.8b`/`.16b` arrangement, negative shift).
- **Expected:** `Err`.
- **Actual:**
  ```
  thread '...' panicked at src/backend/arm/assembler/encoder/neon.rs:1271:25:
  attempt to add with overflow
  ```

GNU `as` / `llvm-mc` reject negative shift immediates with "immediate out of range".

## Impact

Assembler crash: any `sli …, #-N` aborts the whole assembler in debug builds
instead of reporting an error, denying service on otherwise-recoverable malformed
input. In release builds (overflow wraps) the masked value is silently
mis-encoded — compounding with the silent-acceptance bug.

## Suggested Fix

Validate the shift against `[0, esize-1]` immediately after reading it, on the
`i64` value (before the `as u32` cast):

```rust
let shift_i = get_imm(operands, 2)?;
let esize = /* per arrangement: 8 / 16 / 32 / 64 */;
if shift_i < 0 || (shift_i as u32) >= esize {
    return Err(format!("sli: shift {} out of range for {}-bit elements", shift_i, esize));
}
let shift = shift_i as u32;
let immh_immb = (esize + shift) & 0x7F;
```

Apply the same check to `encode_neon_shl`, `encode_neon_sri`,
`encode_neon_ushr`, and `encode_neon_sshr` (identical pattern; `encode_neon_shl`
is already tracked in `shift_left_imm-panic-on-negative-shift.md`).

## Regression Property

Failing property: `prop_sli_rejects_out_of_range_shift`

```rust
#[test]
#[ignore = "documented bug: SLI accepts out-of-range shift immediates"]
fn prop_sli_rejects_out_of_range_shift(
    (arr, _q, esize) in arr_strategy(),
    too_big in proptest::sample::select(vec![0u32, 1, 2, 4, 8, 16, 31, 32, 63, 64, 100, 255]),
    is_negative in any::<bool>(),
) {
    let shift: i64 = if is_negative {
        -((too_big % 64) as i64 + 1)
    } else {
        (esize as i64) + (too_big as i64)
    };
    let ops = vec![vreg_arr(0, arr), vreg_arr(0, arr), Operand::Imm(shift)];
    let res = encode_neon_sli(&ops);
    prop_assert!(res.is_err(), /* ... */);
}
```

Minimal failing input (this bug): `arr = "8b", too_big = 0, is_negative = true`
→ shift = -1 → panic `attempt to add with overflow` at `neon.rs:1271`.

**GitHub Issue:** _(none created)_

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/46
