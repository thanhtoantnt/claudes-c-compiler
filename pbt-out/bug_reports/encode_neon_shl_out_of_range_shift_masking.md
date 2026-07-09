# Bug: `encode_neon_shl` silently masks out-of-range shift amounts (UNALLOCATED encodings)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_shl`
**Severity:** High

## Summary

`encode_neon_shl` computes the `immh:immb` field as `(esize + shift) & mask`
with a width-mask per element size, but performs **no range validation** on
`shift`. AArch64 restricts `SHL` (vector) shift amounts to `[0, esize-1]` per
arrangement; the reference assembler rejects anything larger. The encoder
instead wraps the out-of-range value, producing a wrong and often
**UNALLOCATED** 32-bit word with no diagnostic. A user writing `shl v0.8b,
v1.8b, #8` silently gets `immh:immb = 0` (the reserved `immh == 0b000` case)
instead of an error.

## Root Cause

```rust
pub(crate) fn encode_neon_shl(operands: &[Operand]) -> Result<EncodeResult, String> {
    ...
    let shift = get_imm(operands, 2)? as u32;
    let (q, _) = neon_arr_to_q_size(&arr_d)?;

    // SHL Vd.T, Vn.T, #shift
    // 0 Q 0 0 11110 immh:immb 010101 Rn Rd
    // immh:immb = element_size + shift
    let immh_immb = match arr_d.as_str() {
        "8b" | "16b" => (8 + shift) & 0xF,    // <-- mask silently wraps
        "4h" | "8h" => (16 + shift) & 0x1F,
        "2s" | "4s" => (32 + shift) & 0x3F,
        "2d" => (64 + shift) & 0x7F,
        _ => return Err(format!("unsupported shl arrangement: {}", arr_d)),
    };
    ...
}
```

`shift` is never checked against the per-arrangement maximum. Valid ranges
(ARMv8-A ARM, confirmed by `llvm-mc-18`):

| arrangement | esize | valid shift |
|-------------|-------|-------------|
| 8B / 16B    | 8     | 0 ..= 7     |
| 4H / 8H     | 16    | 0 ..= 15    |
| 2S / 4S     | 32    | 0 ..= 31    |
| 2D          | 64    | 0 ..= 63    |

## Reproduction

```
$ llvm-mc-18 -triple=aarch64 -assemble -show-encoding <<< 'shl v0.8b, v1.8b, #8'
<stdin>:1:19: error: immediate must be an integer in range [0, 7].
```

This crate returns `Ok` instead of `Err`:

```
$ cargo test --lib neon_ins_shl_addv_pbt::shl_out_of_range_shift_regression -- --ignored
thread '...::shl_out_of_range_shift_regression' panicked:
  shl v0.8b, v1.8b, #8 must be rejected; got Ok(0x0F005420) (silently masked immh:immb = 0x00)
```

The boundary witnesses (one per element size) all currently emit a word:

| input                         | emitted word | immh:immb | note              |
|-------------------------------|--------------|-----------|-------------------|
| `shl v0.8b, v1.8b, #8`        | `0x0F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.16b, v1.16b, #8`      | `0x4F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.4h, v1.4h, #16`       | `0x0F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.8h, v1.8h, #16`       | `0x4F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.2s, v1.2s, #32`       | `0x0F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.4s, v1.4s, #32`       | `0x4F005420` | `0x00`    | immh=0 UNALLOCATED |
| `shl v0.2d, v1.2d, #64`       | `0x4F005420` | `0x00`    | immh=0 UNALLOCATED |

The masking is also *non-monotonic*: a large overshoot wraps back to a
**valid-looking** encoding. E.g. `shl v0.8b, v1.8b, #16` gives `immh:immb = 8`,
the encoding of `#0` — a completely different instruction.

## Impact

High — silent mis-assembly. An out-of-range shift produces either an
UNALLOCATED word or a valid encoding for a *different* shift amount, with no
diagnostic. This diverges from the reference assembler and the programmer's
intent.

### Unverified lead (NOT part of the confirmed finding above)

By source reading, a negative `#shift` is `get_imm(...) as u32`-cast
(e.g. `-1 → 0xFFFFFFFF`) *before* `8 + shift`, which would overflow-and-panic
in debug builds. This was **not** exercised by the property suite (generators
draw non-negative shifts), so it is a follow-up lead, not a confirmed bug.

## Suggested Fix

Validate `shift` against the per-arrangement maximum before building the word:

```rust
let esize: u32 = match arr_d.as_str() {
    "8b" | "16b" => 8,
    "4h" | "8h" => 16,
    "2s" | "4s" => 32,
    "2d" => 64,
    _ => return Err(format!("unsupported shl arrangement: {}", arr_d)),
};
let shift = get_imm(operands, 2)?;
if shift < 0 || (shift as u32) >= esize {
    return Err(format!("shl: shift {shift} out of range for .{arr_d} (0..={})", esize - 1));
}
let immh_immb = (esize + shift as u32);   // mask no longer needed
```

After the fix the `#[ignore]`d witnesses `shl_rejects_out_of_range_shift` and
`shl_out_of_range_shift_regression` pass and can be un-ignored.

## Regression Property

Confirmed by the **failing, shrunk `proptest!` property**
`shl_rejects_out_of_range_shift` (module
`backend::arm::assembler::encoder::neon_ins_shl_addv_pbt`, `#[ignore]`d):

- **Shrunk counterexample (minimal failing input):** `arr = "8b", over = 0`
  ⇒ `shl v0.8b, v1.8b, #8`.
- **reproduce:** `cargo test --lib neon_ins_shl_addv_pbt::shl_rejects_out_of_range_shift -- --ignored`

A deterministic boundary companion `shl_out_of_range_shift_regression`
(also `#[ignore]`d) covers one-past-max for every element size:

```rust
#[test]
#[ignore]
fn shl_out_of_range_shift_regression() {
    for &(arr, shift) in &[
        ("8b", 8u32), ("16b", 8), ("4h", 16), ("8h", 16),
        ("2s", 32), ("4s", 32), ("2d", 64),
    ] {
        let ops = vec![va(0, arr), va(1, arr), imm(shift as i64)];
        assert!(encode_neon_shl(&ops).is_err(),
            "shl v0.{arr}, v1.{arr}, #{shift} must be rejected");
    }
}
```
