# Bug Report: `encode_cnt` silently accepts undefined NEON arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_cnt`
**Severity:** Medium

## Summary

`encode_cnt` validates neither destination nor source register arrangement. ARMv8 ARM defines `CNT` only for `.8b` (Q=0) and `.16b` (Q=1); all other arrangements are UNDEFINED. Implementation tests only `arr_d == "16b"` and falls through to `Q=0` for anything else.

## Root Cause

```rust
let q: u32 = if arr_d == "16b" { 1 } else { 0 }; // .8b -> Q=0, .16b -> Q=1
// No check that arr_d is one of {"8b", "16b"}
```

Source arrangement `_arr_n` is read and discarded entirely.

## Reproduction

**Input:** `cnt v0.4h, v0.8b`

**Expected:** `Err` — cnt: arrangement must be .8b or .16b, got .4h

**Actual:** `Ok(Word(0x0E205800))` — identical to `cnt v0.8b, v0.8b` (silently coerced)

**Minimal failing input:** dest="4h", src="8b"

## Impact

`cnt v0.4h, v0.8b` produces valid-looking word with no diagnostic. User silently given different semantics than written.

## Suggested Fix

Reject arrangements outside {"8b","16b"}:

```rust
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("cnt: arrangement must be .8b or .16b, got .{}", arr_d));
}
```

## Regression Property

Failing property: `prop_cnt_rejects_non_byte_arrangements`

```rust
prop_assert!(encode_cnt(&[neon_reg(0, "4h"), neon_reg(0, "8b")]).is_err());
prop_assert!(encode_cnt(&[neon_reg(0, "8h"), neon_reg(0, "8b")]).is_err());
prop_assert!(encode_cnt(&[neon_reg(0, "2s"), neon_reg(0, "8b")]).is_err());
```

## PBT Results (module `neon_cnt_pbt`)

| Property | Result |
|---|---|
| `prop_cnt_matches_arm_reference` | PASS |
| `prop_cnt_fields_isolated` | PASS |
| `prop_cnt_q_and_fixed_bits` | PASS |
| `golden_cnt_matches_llvm_mc` | PASS |
| `rejects_too_few_operands` | PASS |
| `prop_cnt_rejects_non_byte_arrangements` | **FAIL** |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/173