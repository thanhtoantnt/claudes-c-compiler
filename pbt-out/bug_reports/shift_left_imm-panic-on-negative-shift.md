# Bug — `encode_neon_shift_left_imm` panics on negative shift immediate

**File:** `src/backend/arm/assembler/encoder/neon.rs:1768`
**Function:** `pub(crate) fn encode_neon_shift_left_imm(operands, u, opcode)`
**Reached via:** `SQSHL` / `UQSHL` (`encoder/mod.rs:600,605`).
**Discovering property:** `neon_shift_left_imm_pbt::prop_shift_left_imm_rejects_out_of_range_shift` (EXPECTED-FAIL).

## Minimal input

```
sqshl v0.8b, v0.8b, #-1          (equivalently uqshl ... , #-1)
operands = [ RegArrangement(v0,"8b"), RegArrangement(v0,"8b"), Imm(-1) ]
```

## Expected vs. actual

- **Expected:** `Err(...)`. A negative shift is outside the valid range
  `0 <= shift <= esize-1` for the AArch64 "Advanced SIMD shift by immediate"
  left-shift class and must be rejected.
- **Actual:** **panic** in debug builds:

```
panicked at src/backend/arm/assembler/encoder/neon.rs:1768:17: attempt to add with overflow
```

  In release builds the overflow wraps and the function returns a word with
  `immh = 0000`, a RESERVED/UNDEFINED encoding, instead of `Err`.

## Root cause

```rust
let shift = get_imm(operands, 2)? as u32;   // -1i64 -> 0xFFFF_FFFF
...
let immhb = esize + shift;                  // line 1768: 8 + 4294967295 -> overflow panic
```

`get_imm` returns `i64`; `as u32` silently wraps negatives into huge u32 values,
and `esize + shift` is never bounds-checked before the addition.

## Impact

Any negative immediate on `SQSHL`/`UQSHL` aborts the assembler process (debug /
test builds) or emits a reserved instruction word (release builds). This is a
robustness/DoS-grade defect for an assembler entry point that parses
attacker-/user-controllable input.

## Fix

Validate before use:

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 0 || shift_i as u32 >= esize {
    return Err(format!(
        "shift left imm: shift {} out of range [0,{}] for {}-bit elements",
        shift_i, esize - 1, esize
    ));
}
let shift = shift_i as u32;
```
