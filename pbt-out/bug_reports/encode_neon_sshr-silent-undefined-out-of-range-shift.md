# `encode_neon_sshr` accepts out-of-range shifts, emitting reserved (UNDEFINED) words

**Function:** `encode_neon_sshr` — `src/backend/arm/assembler/encoder/neon.rs`
**Tests:** `src/backend/arm/assembler/encoder/neon_sshr_pbt.rs`
**Severity:** Medium
**Witness test:** `sshr_rejects_out_of_range_shift` (`#[ignore]`d; FAILING proptest, run with `--ignored`)

## Minimal input

```
sshr v0.8b, v1.8b, #0      -> returns Ok(Word(0x0F000420))   [251659264]
sshr v0.8b, v1.8b, #9      -> returns Ok(Word(0x0F070420))
sshr v0.4s, v1.4s, #33     -> returns Ok(Word(0x0F370420))   (immh=0b0011)
```

General form: any `shift` with `0` or `esize < shift <= 2*esize` for the
arrangement's element size `esize` (8/16/32/64).

## Expected vs. actual

**Expected** (matches `llvm-mc-14` and the ARMv8-A ARM, "SSHR (vector)"):
the shift must satisfy `1 <= shift <= esize`; out-of-range values must be
rejected. `llvm-mc-14 -assemble -arch=aarch64 -mattr=+neon` reports:

```
sshr v0.8b, v1.8b, #0   -> error: immediate must be an integer in range [1, 8].
sshr v0.8b, v1.8b, #9   -> error: immediate must be an integer in range [1, 8].
sshr v0.4s, v1.4s, #33  -> error: immediate must be an integer in range [1, 32].
```

**Actual:** `encode_neon_sshr` performs **no range validation** and returns
`Ok(EncodeResult::Word(...))`. For these inputs `immh` (the top nibble of
`immh:immb`) collapses to `0b0000`, which the ARM ARM reserves as UNDEFINED for
the shift-by-immediate class:

```rust
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,   // shift in (8,16] -> immh = 0
    "4h" | "8h"  => (32 - shift) & 0x1F,
    "2s" | "4s"  => (64 - shift) & 0x3F,  // shift = 33 -> 0x37, immh = 0b0011 (16-bit elem!)
    "2d"         => (128 - shift) & 0x7F,
    ...
};
```

proptest witness output:

```
reproduce= cargo test --lib -- --ignored neon_sshr_pbt::sshr_rejects_out_of_range_shift
  Falsifiable / minimal failing input: rd = 0, rn = 0, arr = "8b", extra = 1
  Test failed: sshr 8b shift 0 is out of range [1, 8] but was accepted as Some(Word(251659264))
```

## Impact

A malformed operand in source text becomes a 32-bit word the hardware may
decode as a **different** instruction (e.g. `.2s #33` silently re-encodes as a
16-bit-element shift) or raise an UNDEFINED exception — instead of failing the
assembly with a clear error. Same defect class as `encode_neon_ushr` and
`encode_neon_sri` (the signed/unsigned siblings).

## Fix

Bound-check the immediate before computing `immh:immb`:

```rust
let shift = get_imm(operands, 2)?;
if !(1..=esize).contains(&(shift as u32)) {
    return Err(format!("sshr: shift {} out of range [1, {}]", shift, esize));
}
let immh_immb = 2 * esize - (shift as u32); // masking no longer needed
```
