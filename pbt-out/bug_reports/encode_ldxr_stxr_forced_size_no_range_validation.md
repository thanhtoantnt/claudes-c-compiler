# Bug — `encode_ldxr_stxr` silently truncates out-of-range `forced_size`

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `pub(crate) fn encode_ldxr_stxr(operands, is_load, forced_size: Option<u32>) -> Result<EncodeResult, String>`
**Test module:** `prop_encode_ldxr_stxr_tests` (appended to the same file)
**Framework:** `proptest` (already a dev-dependency)
**Result:** 4/5 properties PASS, **1 FAILS** → this bug.

## Summary

`encode_ldxr_stxr` never validates `forced_size` against the 2-bit `size`
encoding field (`[31:30]`). When `forced_size ≥ 4`, the line

```rust
let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 });
let word = (size << 30) | …
```

computes `size << 30` with `size ≥ 4`. Because the shift discards bits above
`[31]`, the value **wraps modulo 4** and silently lands in the `size` field,
producing the encoding for a *different* instruction.

## Reproduction

```
cargo test --lib prop_encode_ldxr_stxr_tests::prop_forced_size_out_of_range_rejected
```

Minimal failing input: `forced_size = Some(4)`.

- Expected: `Err` (4 is outside the 2-bit `size` range 0..=3).
- Actual:   `Ok(Word(140475424))` where `140475424 == 0x085F7C20`.

`0x085F7C20` is exactly the **LDXRB** encoding (size=`0b00`, byte load) — the
canonical `ldxr x0,[x1]` is `0xC85F7C20` (size=`0b11`). So `forced_size=4`
wraps `size` to `0b00` and aliases the byte form. The wrap table is:

| `forced_size` | resulting `size` `[31:30]` | aliased instruction |
|--------------:|---------------------------:|---------------------|
| 4             | `0b00`                      | LDXRB / STXRB (byte) |
| 5             | `0b01`                      | LDXRH / STXRH (half) |
| 6             | `0b10`                      | LDXR  / STXR  (32-bit) |
| 7             | `0b11`                      | LDXR  / STXR  (64-bit) |

## Severity

**Latent / low immediate impact, high correctness hazard.** Every current
caller (`mod.rs:348-353`) passes only `None`, `Some(0b00)`, or `Some(0b01)`,
so today's assembler output is correct. However the function does not defend
its own invariant: any future caller (a new mnemonic handler, a refactor, or a
programmatic user of the encoder) that passes an out-of-range `size` will get
a *silently wrong* instruction that disassembles as a valid, different op —
the worst class of assembler bug (no error, no panic, miscompiled code).

This matches the recurring encoder anti-pattern flagged for this codebase:
immediates / sizes / lane fields are fed straight into bit-shifts with no range
guard, so overflow is truncation rather than rejection.

## Spec basis

ARMv8-A ARM, LDXR/STXR encodings: `size` is a 2-bit field; only `00/01/10/11`
are allocated (byte/half/32/64). No architectural wrapping is defined, so the
correct behaviour for an out-of-range `size` is to reject the input.

## Suggested fix

Validate `forced_size` before use (mirrors the range check already applied to
register numbers in `parse_reg_num`):

```rust
let size = match forced_size {
    Some(s) if s <= 0b11 => s,
    Some(s) => return Err(format!("ldxr/stxr: size out of range (0..=3): {}", s)),
    None => if is_64 { 0b11 } else { 0b10 },
};
```

With that guard the negative-contract property
(`prop_forced_size_out_of_range_rejected`) passes, and the four golden /
field-placement properties continue to pass unchanged.

## Properties summary

| # | Name | Oracle type | Status | What it pins down |
|---|------|-------------|--------|-------------------|
| 1 | `prop_ldxr_golden_and_fields` | Reference (golden `0xC85F7C20`) | PASS | Constant bits, Rt[4:0], Rn[9:5], reserved Rs[20:16]=11111 & Rt2[14:10]=11111, size=11. |
| 2 | `prop_stxr_golden_and_fields` | Reference (golden `0xC8007C20`) | PASS | Rs status[20:16], Rt, Rn, reserved Rt2=11111, constant bits. |
| 3 | `prop_size_field_auto_and_override` | Differential | PASS | `size[31:30]` follows `forced_size`; auto-detect X→11 / W→10. |
| 4 | `prop_load_store_discriminator` | Differential | PASS | LDXR sets L bit [22]=1, STXR clears [22]=0; both o2[21]=0 (single-reg). |
| 5 | `prop_forced_size_out_of_range_rejected` | Negative / error contract | **FAIL** | `forced_size ≥ 4` must return `Err`; instead silently truncates → aliases another op. |

Golden values cross-checked against the ARMv8-A ARM:
`ldxr x0,[x1] = 0xC85F7C20`, `stxr w0,x0,[x1] = 0xC8007C20`.
