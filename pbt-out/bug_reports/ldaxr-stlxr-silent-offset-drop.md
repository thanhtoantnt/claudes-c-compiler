# Bug Report — `encode_ldaxr_stlxr` silently drops non-zero `[Xn, #imm]` offset

## Target
`src/backend/arm/assembler/encoder/load_store.rs::encode_ldaxr_stlxr`
(definition at line 573)

## Summary
`encode_ldaxr_stlxr` matches the memory operand with `Operand::Mem { base, .. }`,
**silently discarding the `offset` field**. Per the ARMv8-A Architecture Reference
Manual (§C4.1.49 LDAXR, §C4.1.116 STLXR), the exclusive load/store register
instructions support **only** the `[Xn]` addressing form — there is no
immediate-offset, pre-index, or post-index encoding. An operand such as
`[x1, #8]` is therefore not representable and should be rejected. Instead the
encoder emits the offset-0 `[Xn]` encoding without warning, silently accepting an
invalid instruction and losing the user's intended displacement.

## Evidence — failing property-based test
`prop_encode_ldaxr_stlxr_tests::prop_nonzero_offset_rejected` fails on its first
input (`off = 1`, `is_load = false`):

```
offset 1 on LDAXR/STLXR is not encodable (only [Xn] is legal) and must be
rejected, but the encoder silently produced Ok(Word(3355507777))
```

`3355507777 == 0xC801FC40`, i.e. the encoder emitted `stlxr w0, x1, [x2]`
(offset 0) for the illegal input `stlxr w0, x1, [x2, #1]`.

## Root cause (load_store.rs:577 and 588)
```rust
// load path (line 577)
Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
// store path (line 588)
Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
```
The `..` ignores `offset`. The same bug pattern (`Mem { base, .. }`) appears in
the sibling exclusive instructions `encode_ldxr_stxr` (551/562),
`encode_ldxp_stxp` (611/625), and `encode_ldar_stlr` (640).

## Severity
**Medium** — silent acceptance of an unrepresentable instruction. The emitted
bytes are a *different* (valid) instruction than the source requested, so the
assembler produces wrong code with no diagnostic. Worst case: a load/store at the
wrong address in hand-written atomics.

## Suggested fix
Reject any non-zero offset in the memory operand, e.g.:
```rust
Some(Operand::Mem { base, offset }) if *offset == 0 => parse_reg_num(base).ok_or("invalid base")?,
_ => return Err("ldaxr/stlxr supports only [Xn] addressing (no offset)".to_string()),
```

## Test-suite status
Module `prop_encode_ldaxr_stlxr_tests` (already present in the file) — 5 properties:

| Property | Result |
|---|---|
| `prop_layout_vs_golden_and_l_bit` (llvm-mc golden field layout + L-bit differential) | PASS |
| `prop_fixed_bits_o0_and_reserved` (o0 acquire/release bit, reserved fields) | PASS |
| `prop_size_field_auto_and_forced` (size[31:30] width auto-detect + forced_size) | PASS |
| `prop_malformed_operands_rejected` (negative contract: wrong operand types) | PASS |
| `prop_nonzero_offset_rejected` (negative contract: this bug) | **FAIL** |

The four passing properties confirm the **encoding core is correct**: all four
load size variants (`ldaxr`/`ldaxrb`/`ldaxrh`, `w`/`x`) and all four store size
variants (`stlxr`/`stlxrb`/`stlxrh`) match `llvm-mc-18` goldens exactly
(`0xC85FFC20`, `0x885FFC20`, `0x085FFC20`, `0x485FFC20`; `0xC801FC40`,
`0x8801FC40`, `0x0801FC40`, `0x4801FC40`). The sole defect is the silent offset drop.

## Build note (incidental fix)
The crate's `cargo test` was blocked by three test modules in
`src/backend/arm/assembler/encoder/data_processing.rs` (`negs_props`,
`sxtb_props`, `uxth_props`) that were missing the `#[cfg(test)]` attribute every
sibling has, so they compiled into the lib build where the `proptest`
dev-dependency is unavailable. Added the three missing `#[cfg(test)]` gates
(no behavior change) to unblock test execution.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/157
