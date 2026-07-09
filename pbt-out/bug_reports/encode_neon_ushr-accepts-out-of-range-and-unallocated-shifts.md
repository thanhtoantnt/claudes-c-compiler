# Bug — `encode_neon_ushr` silently accepts out-of-range / unallocated shift amounts

Target: `encode_neon_ushr` in
`src/backend/arm/assembler/encoder/neon.rs`.

Status: **confirmed** by the `#[ignore]`d property-based witness
`neon_ushr_pbt::prop_out_of_range_shifts_must_be_rejected` in
`src/backend/arm/assembler/encoder/neon_ushr_pbt.rs`. Default `cargo test`
stays green; reproduce with `cargo test neon_ushr_pbt -- --ignored`.

## Affected code

```rust
let shift = get_imm(operands, 2)? as u32;
let (q, _) = neon_arr_to_q_size(&arr_d)?;
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,
    "4h" | "8h" => (32 - shift) & 0x1F,
    "2s" | "4s" => (64 - shift) & 0x3F,
    "2d" => (128 - shift) & 0x7F,
    _ => return Err(format!("unsupported ushr arrangement: {}", arr_d)),
};
```

Only the **arrangement** is validated; the **shift amount** is never checked.

## Spec

ARMv8 ARM, "Advanced SIMD shift by immediate": USHR requires
`1 <= shift <= esize` (esize ∈ {8, 16, 32, 64}), and `immh == 0b0000` is
**UNALLOCATED**.

## Minimal failing input

```text
ushr v0.8b, v1.8b, #0      // arrangement "8b", shift = 0
```

## Expected vs. actual

| Input (`shift`) | Expected | Actual |
|---|---|---|
| `0` (any size) | `Err` (immh would be `0000` → UNALLOCATED) | `Ok(word)` with `immh == 0` |
| `esize + 1` (e.g. `.8b` with `#9`) | `Err` (out of range) | `Ok(word)` — `(16 - 9) = 7 = 0b0111`, so `immh` collapses to `0`; the word masquerades as a different, undefined shift encoding |

Witness stdout:
> `Test failed: shift=0 must be rejected for 8b (immh would be 0)`

## Impact

A malformed `#shift` in the assembler input is accepted and emitted as a word
the CPU cannot decode (or as a semantically different instruction). Silent
mis-compilation of the user's `ushr` mnemonic.

## Suggested fix

Validate the shift against the element size before encoding:

```rust
let (immh_immb, max) = match arr_d.as_str() {
    "8b" | "16b" => (16u32.wrapping_sub(shift) & 0xF, 8u32),
    "4h" | "8h"  => (32u32.wrapping_sub(shift) & 0x1F, 16u32),
    "2s" | "4s"  => (64u32.wrapping_sub(shift) & 0x3F, 32u32),
    "2d"         => (128u32.wrapping_sub(shift) & 0x7F, 64u32),
    _ => return Err(format!("unsupported ushr arrangement: {}", arr_d)),
};
if shift == 0 || shift > max {
    return Err(format!("ushr: shift {} out of range [1, {}] for {}", shift, max, arr_d));
}
```

## Notes

* The same defect exists in the older generalized encoder
  `encode_neon_shift_imm` (already documented in its own PBT file).
  `encode_neon_ushr` inherits the bug verbatim.
* Positive properties in the same PBT file confirm the encoding is otherwise
  correct for all valid shifts in `[1, esize]`.
