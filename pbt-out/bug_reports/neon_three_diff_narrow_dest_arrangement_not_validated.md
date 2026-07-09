# Bug Report: `encode_neon_three_diff_narrow` destination arrangement not validated

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_three_diff_narrow`
**Severity:** Medium

## Summary

`encode_neon_three_diff_narrow` (ADDHN/RADDHN/SUBHN/RSUBHN family) derives the entire encoding from the **source** arrangement (`arr_n`, operand 1) and discards the destination arrangement entirely. A mismatched or missing destination arrangement is silently accepted, producing the same word as the correct narrow destination. The reference assembler (`clang --target=aarch64`) rejects both cases.

## Root Cause

```rust
let (rd, _) = get_neon_reg(operands, 0)?;   // arrangement discarded, never validated
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let (rm, _) = get_neon_reg(operands, 2)?;
let size = match arr_n.as_str() { "8h" => 0b00u32, "4s" => 0b01, "2d" => 0b10, ... };
```

The destination register's arrangement is never checked against the expected narrow type for the source.

## Reproduction

**Input:** `addhn v0.4s, v1.8h, v2.8h` (destination should be `.8b`, not `.4s`)

**Expected:** `Err` — destination arrangement must be `.8b` for `.8h` sources

**Actual:** `Ok(Word(0x0E224020))` — identical to the valid `addhn v0.8b, v1.8h, v2.8h`

**Minimal failing input:** bad_dest = "4s" with source = "8h"

## Impact

Mismatched destination arrangements silently accepted without diagnostic. The wrong-looking instruction emits correct bytes, defeating assembler-level validation. Same "trust the parser" pattern as the sibling widening encoder.

## Suggested Fix

After computing `size`/`Q` from source, validate the destination arrangement matches the expected narrow type:

```rust
let expected_dest = match arr_n.as_str() {
    "8h" => if is_high { "16b" } else { "8b" },
    "4s" => if is_high { "8h" } else { "4h" },
    "2d" => if is_high { "4s" } else { "2s" },
    _ => return Err(format!("invalid source arrangement: {}", arr_n)),
};
if arr_d != expected_dest {
    return Err(format!("narrowing: dest must be .{} for .{} source", expected_dest, arr_n));
}
```

## Regression Property

Failing property: `narrow_rejects_inconsistent_destination_arrangement`

```rust
prop_assert!(encode_neon_three_diff_narrow(
    &[vreg_arr(0, "4s"), vreg_arr(1, "8h"), vreg_arr(2, "8h")], 0, 0b0100, false
).is_err());  // dest .4s invalid for .8h source
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/10
