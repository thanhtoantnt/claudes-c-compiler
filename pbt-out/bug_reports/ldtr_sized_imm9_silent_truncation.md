# BUG: `encode_ldtr_sized` silently truncates out-of-range LDTR/STTR immediates

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `encode_ldtr_sized(operands, is_load, size)`
**Severity:** High — wrong machine code emitted with no diagnostic
**Found by:** property `prop_out_of_range_imm9_is_rejected` (expected-fail negative contract)

## Summary

The LDTR/STTR `imm9` field is a **signed 9-bit** immediate with valid range
`[-256, 255]` (ARMv8 ARM §C4.1.66). The encoder masks out-of-range offsets
into the 9-bit field instead of rejecting them:

```rust
let imm9_enc = (imm9 as u32) & 0x1FF;
let word = (size << 30) | (0b111 << 27) | (opc << 22)
    | (imm9_enc << 12) | (0b10 << 10) | (rn << 5) | rt;
Ok(EncodeResult::Word(word))
```

`& 0x1FF` silently wraps any offset, e.g. `#256 → #0`, `#512 → #0`,
`#257 → #1`, `#-257 → #-1`, `#-512 → #0`. The assembler emits a *correct*
but *wrong* instruction word and returns `Ok`, so the error is invisible to
the caller and the resulting program silently accesses the wrong address.

## Reproduction

```
minimal failing input: excess = 1, negative = false
   → offset = 256 (just past the valid +255)
   → encoder returned Ok(Word(0xF8400820))
   → 0xF8400820 is exactly `ldtr x0, [x1, #0]`  (#256 became #0)
```

## Expected behaviour

Return `Err` for `offset < -256 || offset > 255`, e.g.:

```rust
if imm9 < -256 || imm9 > 255 {
    return Err(format!("ldtr/sttr: imm9 offset {} out of range [-256, 255]", imm9));
}
```

(Consistent with how `encode_prfm` already validates its immediate range in
this same file.)

## Verification of the finding

Run:

```
cargo test --lib prop_encode_ldtr_sized_tests
```

Result: **6 passed, 1 failed**.

| Property | Result | Checks |
|---|---|---|
| `prop_gp_layout_matches_golden` | ✅ pass | full-word field layout vs golden `0xF8400820` |
| `prop_load_xor_store_is_opc_bit22` | ✅ pass | `load ^ store == 0x0040_0000` |
| `prop_size_param_in_top_two_bits` | ✅ pass | `size` lands in `[31:30]` |
| `prop_imm9_field_sign_extended_equals_input` | ✅ pass | imm9 round-trips for in-range offsets; pins goldens at 0/+8/-1 |
| `prop_error_contract` | ✅ pass | arity < 2 and non-`Mem` operands → `Err` |
| `golden_ldtr_sttr_encodings` | ✅ pass | exact match to hand-derived ARMv8 words |
| `prop_out_of_range_imm9_is_rejected` | ❌ **FAIL** | out-of-range imm9 must be `Err`, is silently wrapped |

The 6 passing properties confirm the *field placement* of the encoder is
correct — the only defect is the missing range check. This is the same
silent-truncation bug pattern already documented by negative-contract
properties on the sibling functions `encode_ldr_str`, `encode_ldur_stur`,
`encode_ldp_stp`, and `encode_ldnp_stnp` in this same file.
