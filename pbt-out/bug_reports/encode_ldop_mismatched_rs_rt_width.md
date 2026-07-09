# Bug Report: `encode_ldop` silently accepts mismatched `<Rs>`/`<Rt>` register widths

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldop`
**Severity:** Medium

## Summary

For the register (non-`b`/non-`h`) form of every LSE atomic memory op (LDADD/LDCLR/LDEOR/LDSET and their `a`/`l` variants), the ARMv8.1-A ARM requires `<Rs>` and `<Rt>` to be the **same** register width (both `W` or both `X`); a width mismatch is UNPREDICTABLE and assemblers reject it. `encode_ldop` derives the `size` field exclusively from `<Rs>` (operands[0]) via `get_reg(...)?.1` and binds `<Rt>`'s width to `_`, so `ldadd w0, x1, [x2]` is silently encoded as a 32-bit op (`size=10`) and `ldadd x0, w1, [x2]` as a 64-bit op (`size=11`), with `<Rt>`'s width ignored and no diagnostic.

## Root Cause

```rust
let (rs, is_64) = get_reg(operands, 0)?;   // is_64 taken from Rs
let (rt, _)     = get_reg(operands, 1)?;   // Rt's width discarded (bound to `_`)
...
let size = if suffix.contains('b') { 0b00 }
           else if suffix.contains('h') { 0b01 }
           else if is_64 { 0b11 }      // <- driven by Rs only
           else { 0b10 };
```

`<Rt>`'s width is decoded by `get_reg` and immediately thrown away; nothing compares it to `<Rs>`'s width before `size` is chosen.

## Reproduction

**Input:** `ldadd w0, x1, [x2]`  (and `ldadd x0, w1, [x2]`)

**Expected:** `Err` — `<Rs>` and `<Rt>` must be the same register width

**Actual:** `Ok(Word(3089104896))` = `0xB8200041` (`size=10`, i.e. the 32-bit W form — `<Rt>`'s X width ignored)

**Minimal failing input:** `mn = "ldadd", rs = 0, rt = 0, rn = 0, rs_is64 = false` (i.e. `ldadd w0, x0, [x0]`)

## Impact

A mismatched-width LSE atomic is silently encoded with the wrong operation size. Because the load/store width determines how many bytes are atomically read/modified/written, a `W` source applied to a 64-bit field (or vice-versa) produces a narrower/wider atomic access than intended, corrupting the surrounding memory transaction with no assembler diagnostic. Severity is Medium rather than High because the common assembler input already matches widths; this bites only on erroneous or hand-written assembly.

## Suggested Fix

Validate width agreement for the register form (byte/half forms have no W/X choice to mismatch):

```rust
let (rs, rs_is64) = get_reg(operands, 0)?;
let (rt, rt_is64) = get_reg(operands, 1)?;
...
if !suffix.contains('b') && !suffix.contains('h') && rs_is64 != rt_is64 {
    return Err(format!(
        "{}: <Rs> and <Rt> must be the same register width", mnemonic
    ));
}
```

## Regression Property

Failing property: `prop_mismatched_width_rejected` (module `load_store_ldop_pbt`, `#[ignore]`d so the default `cargo test` stays green).

```rust
// ldadd w0, x0, [x0] must be Err — Rs/Rt widths differ
prop_assert!(encode_ldop("ldadd", &[wreg(0), xreg(0), mem(0, 0)]).is_err());
```

## PBT Results (module `load_store_ldop_pbt`)

| Property | Result (default / `--ignored`) |
|---|---|
| `prop_reference_encoding_matches` | PASS |
| `prop_mismatched_width_size_follows_rs` | PASS (documents the mechanism) |
| `prop_mismatched_width_rejected` | ignored → **FAIL** under `--ignored` |
| `golden_encodings_match_reference` | PASS |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/274
