# Bug Report: `encode_ldop` silently drops non-zero immediate offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldop`
**Severity:** High

## Summary

`encode_ldop` matches its memory operand with `Operand::Mem { base, .. }` and **silently discards the offset field**. Per the ARMv8.1-A ARM (§C6.2.100 LDADD et seq.), the LSE atomic memory-op group (LDADD/LDCLR/LDEOR/LDSET and variants) has **no immediate-offset addressing form** — the only permitted syntax is `LDop <Rs>, <Rt>, [<Xn|SP>]`, with the effective address exactly equal to the base register. An operand like `[x2, #8]` is unrepresentable and should be rejected. Instead, the encoder treats `ldadd x0, x1, [x2, #8]` identically to `ldadd x0, x1, [x2]` — wrong address, no diagnostic. This is the same defect family already reported for `encode_ldxr_stxr` / `encode_ldxp_stxp` / `encode_ldar_stlr` / `encode_cas` / `encode_swp` in this file.

## Root Cause

```rust
let rn = match operands.get(2) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("ldop: invalid base")?,
    _ => return Err(format!("{} requires memory operand [Xn]", mnemonic)),
};
```

The `..` pattern discards `offset`; it is never inspected. The resulting encoding has no offset field at all (`size 111000 A R 1 Rs 0 opc 00 Rn Rt`), so a stray offset reaches neither the word nor an error path.

## Reproduction

**Input:** `ldadd x0, x1, [x2, #-1]`  (also `[x2, #8]`, `[x2, #4096]`, …)

**Expected:** `Err` — LSE atomics take no immediate offset (`[Xn]` only)

**Actual:** `Ok(Word(4162846785))` = `0xF8200041` — identical to `ldadd x0, x1, [x2]` (offset silently dropped)

**Minimal failing input:** `mn = "ldadd", off = -1`

## Impact

Incorrect code generation for any source writing a non-zero offset on an LSE atomic instruction, with no assembler diagnostic. The emitted load/store targets `[Xn]` instead of `[Xn, #off]`, so an atomic intended for an adjacent field silently operates on the base — a silent memory-ordering/data-corruption bug invisible at assembly time.

## Suggested Fix

Inspect and reject a non-zero offset:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "{}: LSE atomics take no immediate offset (got #{}); use [Xn] only",
            mnemonic, offset
        ));
    }
    parse_reg_num(base).ok_or("ldop: invalid base")?
}
```

## Regression Property

Failing property: `prop_nonzero_offset_rejected` (module `load_store_ldop_pbt`, `#[ignore]`d so the default `cargo test` stays green).

```rust
// ldadd x0, x1, [x2, #-1] must be Err — LSE atomics have no offset form
prop_assert!(encode_ldop("ldadd", &[xreg(0), xreg(1), mem(2, -1)]).is_err());
```

## PBT Results (module `load_store_ldop_pbt`)

| Property | Result (default / `--ignored`) |
|---|---|
| `prop_reference_encoding_matches` | PASS |
| `prop_offset_is_silently_dropped` | PASS (documents the mechanism) |
| `prop_nonzero_offset_rejected` | ignored → **FAIL** under `--ignored` |
| `golden_encodings_match_reference` | PASS |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/273
