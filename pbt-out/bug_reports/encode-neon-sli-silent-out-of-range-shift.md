# Bug Report: `encode_neon_sli` silently accepts out-of-range shift immediates

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_sli`
**Severity:** Medium

## Summary

`encode_neon_sli` (the AArch64 `SLI` — Shift Left and Insert — encoder) performs
**no range validation** on the `#shift` immediate. The shift is fed straight into
`immh_immb = (esize + shift) & <mask>` with only a bitwise mask applied. Per the
ARMv8 ARM, `SLI` requires `0 <= shift <= esize-1`; values at or above the element
size are UNDEFINED/UNALLOCATED, but the encoder masks them and returns `Ok`. This
is the silent-acceptance facet of the defect (the panic-on-negative facet is
filed separately).

## Root Cause

```rust
let shift = get_imm(operands, 2)? as u32;          // neon.rs:1264 — no range check
...
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (8 + shift) & 0xF,             // neon.rs:1271 — mask, no reject
    "4h" | "8h"  => (16 + shift) & 0x1F,
    "2s" | "4s"  => (32 + shift) & 0x3F,
    "2d"         => (64 + shift) & 0x7F,
    _ => return Err(format!("unsupported sli arrangement: {}", arr_d)),
};
```

An over-large shift wraps instead of being rejected. For `shift = 8` on `.8b`
(valid range `[0,7]`): `immh_immb = (8 + 8) & 0xF = 0` → `immh = 0000`, a
**reserved / UNALLOCATED** encoding (the ARM decode rule
`esize = 8 << HighestSetBit(immh)` is undefined for `immh == 0`). Larger
out-of-range values wrap to a non-zero `immh` and re-encode as a **different
element size** — e.g. `shift = 16` on `.8b` masks to `immh:immb = 8` (`immh =
0001`), decoding back as `esize = 8, shift = 0`, so `sli …,#16` silently becomes
`sli …,#0`.

## Reproduction

Property `prop_sli_rejects_out_of_range_shift`, run via
`cargo test --lib neon_sli_pbt -- --ignored`, fails with the shrunk minimal
input:

- **Input:** `sli v0.8b, v0.8b, #8` (esize = 8; valid range `[0,7]`).
- **Expected:** `Err`.
- **Actual:** `Ok(EncodeResult::Word(788550656))` = `0x2F005400` (`immh = 0000`
  → reserved encoding).

GNU `as` / `llvm-mc` reject this with "immediate out of range".

## Impact

Silent mis-compilation: any `sli` written with an over-large shift is assembled
without diagnostic into either a reserved encoding or an instruction with a
different element size / shift, producing object code that does not match the
source. (No AArch64 assembler installed in this environment; oracle is the ARM
ARM structural layout plus hand-computed golden words, validated by the five
passing properties.)

## Suggested Fix

Validate the shift against `[0, esize-1]` immediately after reading it, before
computing `immh_immb`:

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
`encode_neon_ushr`, and `encode_neon_sshr` (identical pattern).

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
    prop_assert!(
        res.is_err(),
        "shift {} on .{} (esize {}) is out of range [0,{}] and must be rejected, but got {:?}",
        shift, arr, esize, esize.saturating_sub(1), res
    );
}
```

Minimal failing input (this bug): `arr = "8b", too_big = 0, is_negative = false`
→ shift 8 → `Ok(Word(0x2F005400))`.

**GitHub Issue:** _(none created)_

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/47
