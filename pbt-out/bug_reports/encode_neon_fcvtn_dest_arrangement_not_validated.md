# Bug Report: `encode_neon_fcvtn` does not validate destination register arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_fcvtn`
**Severity:** Low

## Summary

`encode_neon_fcvtn` validates the **source** register arrangement (rejecting
anything other than `.4s`/`.2s`/`.2d`) but completely ignores the **destination**
register's arrangement: it is read and immediately discarded. The destination
type of FCVTN/FCVTN2 is fully determined by the source type (single→half:
`.4h`/`.8h`; double→single: `.2s`/`.4s`), so a strict assembler such as GNU `as`
or `llvm-mc` requires the written destination arrangement to *match* the implied
type and rejects mismatches as "operand size mismatch". This encoder accepts any
destination arrangement — including a bare register with no arrangement at all —
and silently produces the encoding implied by the source.

The encoding bits themselves are correct for the (validated) source; the defect is
the missing destination consistency check, which suppresses diagnostics for
typos/mismatches in the destination operand.

## Root Cause

```rust
pub(crate) fn encode_neon_fcvtn(operands: &[Operand], is_high: bool) -> Result<EncodeResult, String> {
    let (rd, _) = get_neon_reg(operands, 0)?;          // <-- dest arrangement discarded
    let (rn, arr_n) = get_neon_reg(operands, 1)?;
    let sz = match arr_n.as_str() { "4s" | "2s" => 0u32, "2d" => 1,
        _ => return Err(format!("fcvtn: unsupported source: {}", arr_n)), };
    let q = if is_high { 1u32 } else { 0 };
    let word = (q << 30) | (0b01110 << 24) | (sz << 22) | (0b10000 << 17)
        | (0b10110 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_neon_reg(operands, 0)` returns `(rd, arr_d)` but `arr_d` is bound to `_` and
dropped. No comparison is ever made between the written destination arrangement
and the arrangement implied by `arr_n` / `is_high`.

## Reproduction

**Input 1 (mismatched arrangement):** `fcvtn v0.16b, v1.4s`

**Expected:** `Err` — FCVTN with a single-precision `.4s` source requires a
half-precision destination (`.4h` for FCVTN / `.8h` for FCVTN2); `.16b` is an
operand-size mismatch.

**Actual:** `Ok(Word(0x0e216800))` — identical to `fcvtn v0.4h, v1.4s`.

**Input 2 (bare destination, no arrangement):** `fcvtn v0, v1.4s`

**Expected:** `Err` — destination must carry a valid arrangement.

**Actual:** `Ok(Word(0x0e216800))` — silently encoded.

**Minimal failing input:** `rd = 0, rn = 0, src = "4s", dest = "4h"`,
`is_high = false`, with a bare `Operand::Reg("v0")` destination → `Ok(Word(0x0E216800))`.

## Impact

A user who writes a wrong destination arrangement (e.g. `fcvtn v0.4s, v1.2d` when
they meant `fcvtn2`, or `fcvtn v0.16b, v1.4s` by mistake, or omits the
arrangement entirely) gets a valid-looking machine word with **no diagnostic**,
believing they assembled what they wrote. Because the source is correctly
validated and the encoding itself is bit-accurate for that source, this is a
diagnostics/robustness defect rather than a silent mis-encoding — hence Low
severity. It does, however, diverge from the behavior of `gas`/`llvm-mc`, which
both reject these inputs.

## Suggested Fix

Validate the destination arrangement against the type implied by the source and
`is_high` before encoding:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let sz = match arr_n.as_str() {
    "4s" | "2s" => 0u32,
    "2d" => 1,
    _ => return Err(format!("fcvtn: unsupported source: {}", arr_n)),
};
let expected_dest = match (arr_n, is_high) {
    ("4s" | "2s", false) => "4h",
    ("4s" | "2s", true)  => "8h",
    ("2d", false) => "2s",
    ("2d", true)  => "4s",
    _ => unreachable!(),
};
if arr_d != expected_dest {
    return Err(format!(
        "fcvtn: destination arrangement .{} does not match source .{} (expected .{})",
        arr_d, arr_n, expected_dest,
    ));
}
```

## Regression Property

Failing property: `prop_fcvtn_rejects_mismatched_dest_arrangement` (in
`src/backend/arm/assembler/encoder/neon_fcvtn_pbt.rs`)

```rust
// A bare destination register (no arrangement) is always a mismatch and
// must be rejected. Currently returns Ok(Word(0x0E216800)).
let bare = vec![Operand::Reg("v0".into()), vreg_arr(0, "4s")];
prop_assert!(encode_neon_fcvtn(&bare, false).is_err());

// A mismatched destination arrangement (.16b for a .4s source) must be
// rejected. Currently returns Ok(Word(0x0E216800)).
let ops = vec![vreg_arr(0, "16b"), vreg_arr(0, "4s")];
prop_assert!(encode_neon_fcvtn(&ops, false).is_err());
```

## PBT Results (module `neon_fcvtn_pbt`)

| Property | Result |
|---|---|
| `prop_fcvtn_matches_arm_reference` | PASS |
| `prop_fcvtn_fields_isolated` | PASS |
| `prop_fcvtn_q_size_and_fixed_bits` | PASS |
| `prop_fcvtn_dest_arrangement_ignored` | PASS (documents the dead input) |
| `prop_fcvtn_rejects_invalid_source_arrangements` | PASS |
| `golden_fcvtn_matches_arm_reference` | PASS |
| `rejects_too_few_operands` | PASS |
| `prop_fcvtn_rejects_mismatched_dest_arrangement` | **FAIL** (regression marker) |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/195
