# `encode_neon_sshr` panics on oversized / negative immediate shifts

**Function:** `encode_neon_sshr` — `src/backend/arm/assembler/encoder/neon.rs`
**Tests:** `src/backend/arm/assembler/encoder/neon_sshr_pbt.rs`
**Severity:** Medium (crashes the whole assembler instead of returning `Err`)
**Witness test:** `sshr_returns_err_on_overflow_shift` (`#[ignore]`d; FAILING proptest, run with `--ignored`)

## Minimal input

```
sshr v0.8b, v1.8b, #17     -> panic: attempt to subtract with overflow
sshr v0.4h, v1.4h, #33     -> panic: attempt to subtract with overflow
sshr v0.4s, v1.4s, #65     -> panic: attempt to subtract with overflow
sshr v0.2d, v1.2d, #129    -> panic: attempt to subtract with overflow
sshr v0.8b, v1.8b, #-1     -> panic: attempt to subtract with overflow
sshr v0.2d, v1.2d, #-7     -> panic: attempt to subtract with overflow
```

General form: `shift > 2*esize`, or any **negative** immediate.

## Expected vs. actual

**Expected:** the encoder should return `Err(...)` for an out-of-range / bogus
immediate, as it does for unknown arrangements and wrong operand counts.

**Actual:** the unsigned subtraction `(2*esize - shift)` underflows and, under
`debug_assertions` (the default `cargo test` profile), triggers a panic that
aborts the entire assembler:

```rust
let shift = get_imm(operands, 2)? as u32;   // -1 -> u32::MAX
...
"8b" | "16b" => (16 - shift) & 0xF,         // 16 - u32::MAX underflows -> panic
```

The witness is a FAILING proptest that asserts the encoder returns `Err`;
in debug builds the call panics inside the property, which proptest reports as
a failure with a shrunk counterexample:

```
reproduce= cargo test --lib -- --ignored neon_sshr_pbt::sshr_returns_err_on_overflow_shift
  Falsifiable / minimal failing input: rd = 0, rn = 0, arr = "8b", extra = 1, neg = 1
  Test failed: attempt to subtract with overflow.   (sshr 8b #17)
```

## Impact

In debug builds (and anywhere `overflow-checks = true`), a single bad mnemonic
aborts the assembler process rather than reporting a diagnostic. In release
builds the subtraction silently wraps, feeding a garbage `immh:immb` into the
word (see the sibling report
`encode_neon_sshr-silent-undefined-out-of-range-shift.md`).

## Fix

Validate `shift` as a signed value before the subtraction:

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 1 {
    return Err(format!("sshr: shift must be >= 1, got {}", shift_i));
}
let shift = shift_i as u32;
let esize = esize_of(&arr_d);
if shift > esize {
    return Err(format!("sshr: shift {} out of range for {}-bit elements", shift, esize));
}
let immh_immb = 2 * esize - shift; // now provably non-underflowing
```

The same guard resolves both the panic (this report) and the UNDEFINED-word
case (sibling report `encode_neon_sshr-silent-undefined-out-of-range-shift.md`).
