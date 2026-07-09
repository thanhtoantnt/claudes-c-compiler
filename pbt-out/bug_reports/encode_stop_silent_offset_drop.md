# Bug Report: `encode_stop` silently drops non-zero immediate offsets

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_stop`
**Severity:** High

## Summary

`encode_stop` (STADD/STCLR/STEOR/STSET and their `b`/`h`/`l` variants — ARMv8.1-A
LSE *store* aliases, ARM ARM §C6.2.274 STADD et seq.) matches its memory operand
with `Operand::Mem { base, .. }` and **silently discards the offset field**.
Per the ARM ARM these store aliases have **no immediate-offset addressing form** —
the only permitted syntax is `STop <Rs>, [<Xn|SP>]`, with the effective address
exactly equal to the base register. An operand like `[x1, #8]` is unrepresentable
and must be rejected. Instead the encoder treats `stadd x0, [x1, #8]` identically
to `stadd x0, [x1]` — wrong address, no diagnostic.

This is the same defect family already reported for the sibling LSE / exclusive
encoders (`encode_ldop`, `encode_swp`, `encode_cas`, `encode_ldxr_stxr`,
`encode_ldxp_stxp`, `encode_ldar_stlr`, `encode_ldaxr_stlxr`), but it had **not**
previously been reported for `encode_stop`. Its inline `prop_encode_stop_tests`
module tests field placement, opc, and bad-operand rejection, but never exercises
a non-zero offset, so the defect went uncaught.

## Root Cause

```rust
let rn = match operands.get(1) {
    Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or_else(|| format!("{}: invalid base", mnemonic))?,
    _ => return Err(format!("{} requires memory operand [Xn]", mnemonic)),
};
```

The `..` pattern discards `offset`; it is never inspected. The emitted encoding
(`size 111000 A R 1 Rs 0 opc 00 Rn Rt`) carries no offset field, so a stray
offset reaches neither the word nor an error path.

## Reproduction

**Input:** `stadd x0, [x1, #1]`  (also `[x1, #8]`, `[x1, #-1]`, `[x1, #4096]`, …)

**Expected:** `Err` — ST* aliases take no immediate offset (`[Xn]` only)

**Actual:** `Ok(Word(4162846783))` = `0xF820003F` — identical to `stadd x0, [x1]`
(offset silently dropped)

**Minimal failing input (shrunk by proptest):** `mn = "stadd", off = 1`

## Impact

Incorrect code generation for any source writing a non-zero offset on an LSE
store-alias instruction, with no assembler diagnostic. The emitted store targets
`[Xn]` instead of `[Xn, #off]`, so an atomic store intended for an adjacent
field silently writes the base address — a silent memory-ordering / data-
corruption bug invisible at assembly time.

## Suggested Fix

Inspect and reject a non-zero offset:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "{}: LSE store aliases take no immediate offset (got #{}); use [Xn] only",
            mnemonic, offset
        ));
    }
    parse_reg_num(base).ok_or_else(|| format!("{}: invalid base", mnemonic))?
}
```

## Regression Property

Failing property: `stop_nonzero_offset_rejected` (module
`load_store_ldst_stop_pbt`, `#[ignore]`d so the default `cargo test` stays green).

Run it with:

```
cargo test --lib load_store_ldst_stop_pbt::stop_nonzero_offset_rejected -- --ignored
```

```rust
// stadd x0, [x1, #1] must be Err — ST* aliases have no offset form
prop_assert!(encode_stop("stadd", &[gp('x', 0), mem(1, 1)]).is_err());
```

## PBT Results (module `load_store_ldst_stop_pbt`)

| Property | Result (default / `--ignored`) |
|---|---|
| `stop_reference_encoding_matches` | PASS |
| `stop_offset_silently_dropped` | PASS (documents the mechanism) |
| `stop_nonzero_offset_rejected` | ignored → **FAIL** under `--ignored` |
| `stop_golden_encodings` | PASS |
