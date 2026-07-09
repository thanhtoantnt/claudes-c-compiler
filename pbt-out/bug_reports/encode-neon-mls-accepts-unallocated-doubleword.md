# Bug Report: `encode_neon_mls` accepts unallocated doubleword arrangements (`.1d` / `.2d`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mls`
**Severity:** High

## Summary

`neon_arr_to_q_size` maps `1d` → `(0, 0b11)` and `2d` → `(1, 0b11)`. In the "Advanced SIMD three same" encoding group, the multiply family (`MUL` / `MLA` / `MLS`) is architecturally defined **only for `size != 0b11`** (ARMv8-A ARM). `encode_neon_mls` makes no check on `size` and emits unallocated encodings instead of returning `Err`.

## Root Cause

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;          // <-- accepts size==0b11
// MLS: 0 Q 1 01110 size 1 Rm 10010 1 Rn Rd (U=1)
let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
    | (rm << 16) | (0b100101 << 10) | (rn << 5) | rd;
```

## Reproduction

**Input:** `mls v3.1d, v4.1d, v5.1d`

**Expected:** `Err` — MLS does not support .1d (size=0b11 is unallocated); valid arrangements are .8b/.16b/.4h/.8h/.2s/.4s

**Actual:** `Ok(Word(0x2EE29420))` — unallocated encoding accepted

**Minimal failing input:** rd=0, rn=1, rm=2, arrangement="1d"

## Impact

Silent mis-assembly: `mls v0.1d, ...` accepted and turned into undefined/unallocated 32-bit instruction word instead of compile-time error. Sibling `encode_neon_mla` has identical defect.

## Suggested Fix

Reject unallocated doubleword size:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 {
    return Err(format!(
        "MLS does not support {arr_d} (size=0b11 is unallocated); \
         valid arrangements are .8b/.16b/.4h/.8h/.2s/.4s"
    ));
}
```

## Regression Property

Failing property: `mls_rejects_doubleword`

```rust
prop_assert!(encode_neon_mls(&[neon_reg(0, "1d"), neon_reg(1, "1d"), neon_reg(2, "1d")]).is_err());
```

## PBT Results (module `neon_mls_pbt`)

| Property | Result |
|---|---|
| Golden table (6 cases, llvm-mc-18 verified) | PASS |
| `matches_reference_encoder` (differential) | PASS |
| `fields_round_trip_and_map_arrangement` | PASS |
| `fixed_bits_are_constant` (incl. `U==1`) | PASS |
| `rejects_unsupported_arrangement` (unknown arrangement strings) | PASS |
| `mls_rejects_doubleword` (the `.1d`/`.2d` contract) | **FAILS** (ignored) |

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/155