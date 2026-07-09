# Bug Report: `encode_sxtw` silently accepts W destination register

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_sxtw`
**Severity:** Medium

## Summary

`SXTW` (sign-extend word) is defined by the ARMv8 ARM **exclusively** as `SXTW <Xd>, <Wn>` — a 64-bit destination is mandatory. `encode_sxtw` never validates the destination width, so a malformed `SXTW Wd, Wn` is silently accepted and emitted as a valid-looking `SBFM Xd, Xn, #0, #31` with `sf=1` (i.e. the bytes of `SXTW Xd, Xn, #0`).

## Root Cause

```rust
let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded: destination width never validated
let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded: source width never validated
let word = ((1u32 << 31) | (0b100110 << 23) | (1 << 22)) | (31 << 10) | (rn << 5) | rd;
```

The `is_64` flag returned by `get_reg` is discarded for both operands.

## Reproduction

**Input:** `sxtw w0, w0`

**Expected:** `Err` — sxtw requires a 64-bit destination register (Xd)

**Actual:** `Ok(Word(0x93407C80))` — encodes as `sxtw x0, x0, #0`

**Minimal failing input:** n = 0 → `encode_sxtw(&[wreg(0), wreg(0)])`

## Impact

- **Silent mis-encoding**: A user typo such as `sxtw w0, w1` compiles to `SBFM x0, x1, #0` instead of being flagged as an error
- **Defeats the purpose of SXTW**: The whole point of 32→64 extension is defeated when destination is 32-bit
- Same pattern affects `encode_smaddl` / `encode_umaddl` / `encode_umulh` which discard `is_64` flags

## Suggested Fix

Validate that the destination is 64-bit:

```rust
let (rd, rd_is_64) = get_reg(operands, 0)?;
if !rd_is_64 {
    return Err("sxtw requires a 64-bit destination register (Xd)".to_string());
}
```

## Regression Property

Failing property: `sxtw_props::sxtw_rejects_w_destination`

```rust
#[test]
fn sxtw_rejects_w_destination(n in 0u32..=31) {
    let ops = vec![wreg(n), wreg(n)];
    prop_assert!(encode_sxtw(&ops).is_err(), "W destination is architecturally invalid for SXTW; got {:?}", encode_sxtw(&ops));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/199