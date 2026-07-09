# Bug: `encode_neon_qshrn` accepts out-of-range narrowing shift amounts

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_qshrn`

## Summary

`encode_neon_qshrn` (the AArch64 *narrowing* saturating right-shift encoder for
`SQSHRN`/`SQSHRN2`, `UQSHRN`/`UQSHRN2`, `SQRSHRN`/`SQRSHRN2`,
`UQRSHRN`/`UQRSHRN2`) rejects too few shifts but the **upper bound is wrong**: it
permits `shift` up to `esize_src` (16/32/64) when the ARMv8 ARM narrowing form
requires `1 ..= esize_src / 2` (8/16/32). Shifts above the legal ceiling are
silently encoded as corrupt words rather than rejected.

## Root cause (single defect)

```rust
let shift = get_imm(operands, 2)? as u32;
let element_bits = match arr_n.as_str() { "8h" => 16u32, "4s" => 32, "2d" => 64, ... };
if shift == 0 || shift > element_bits {                                    // <-- BUG: should be element_bits / 2
    return Err(format!("qshrn: shift {} out of range for {}-bit elements", shift, element_bits));
}
let immhb = element_bits - shift;
```

The bound is `shift > element_bits`; the correct narrowing-form ceiling is
`element_bits / 2`. The derived `immh:immb` (`immhb = element_bits - shift`) is
only well-formed while `shift <= element_bits / 2`. There is exactly one faulty
expression — everything else in the function is correct.

## Minimal failing input

```
rd = 0, rn = 0, arr_n = "8h", shift = 9, u_bit = 0, is_rounding = false, is_high = false
  i.e.   sqshrn v0.8b, v0.8h, #9
```

- **Expected:** `Err` — shift 9 exceeds the `.8h` legal maximum of 8.
- **Actual:** `Ok(EncodeResult::Word(0x0F079400))`. `immhb = 16 - 9 = 7`,
  so `immh = 0000`, which is **UNALLOCATED** for the Advanced SIMD shift group.

## Impact

Any shift in `(esize/2, esize]` produces a corrupt 32-bit word that no real
assembler would emit:

- `sqshrn v0.8b, v0.8h, #9`  → `0x0F079400` (`immh=0000`, UNALLOCATED)
- `sqshrn v0.4h, v0.4s, #17` → `0x0F0F9400` (`immh=0001`, decodes as a
  16-bit `.8h` source instead of the requested 32-bit `.4s` — silent
  instruction-size corruption)

Because the encoder returns `Ok`, downstream consumers that trust it (ELF
writers, disassemblers, JIT paths) emit a valid-looking but semantically wrong
instruction, or an unallocated word that faults at execution. GNU `as` and
`llvm-mc` reject these inputs at assembly time.

## Evidence (added in `neon_qshrn_pbt.rs`)

- **`prop_qshrn_rejects_over_range_shift`** — failing negative-contract
  proptest (left un-`#[ignore]`d as the promoted bug). Generates a shift in
  `(esize/2, esize]` per source width and asserts `Err`. Fails on the minimal
  input above. The message classifies the corruption mode (`immh=0000` vs
  size-category mismatch) for each accepted word.
- **`over_range_shift_9_on_8h_is_unallocated`** — deterministic witness; fails.
- The passing baseline (`prop_qshrn_matches_arm_layout`,
  `prop_qshrn_fields_isolated`, `prop_qshrn_fixed_bits_and_qu`,
  `golden_qshrn_matches_arm_layout`) proves the bit layout is otherwise correct,
  isolating the defect to the single upper-bound expression.

## Suggested fix

```rust
if shift == 0 || shift > element_bits / 2 {
    return Err(format!(
        "qshrn: shift {} out of range for {}-bit source (narrow shift must be 1..={})",
        shift, element_bits, element_bits / 2,
    ));
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/67
