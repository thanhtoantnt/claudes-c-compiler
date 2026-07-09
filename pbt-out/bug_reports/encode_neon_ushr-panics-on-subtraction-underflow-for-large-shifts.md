# Bug — `encode_neon_ushr` panics on subtraction underflow for large shift amounts

Target: `encode_neon_ushr` in
`src/backend/arm/assembler/encoder/neon.rs` (the per-size subtraction at line
~1192).

Status: **confirmed** by the `#[ignore]`d property-based witness
`neon_ushr_pbt::prop_overflowing_shifts_must_not_panic` in
`src/backend/arm/assembler/encoder/neon_ushr_pbt.rs`. Default `cargo test`
stays green; reproduce with `cargo test neon_ushr_pbt -- --ignored`.

## Affected code

```rust
let immh_immb = match arr_d.as_str() {
    "8b" | "16b" => (16 - shift) & 0xF,    // <-- 16u32 - shift underflows when shift > 16
    "4h" | "8h" => (32 - shift) & 0x1F,
    "2s" | "4s" => (64 - shift) & 0x3F,
    "2d" => (128 - shift) & 0x7F,
    _ => return Err(format!("unsupported ushr arrangement: {}", arr_d)),
};
```

The per-size subtraction runs *before* the masking, so the `& 0xF / 0x1F / …`
does **not** protect against underflow — underflow happens first.

## Minimal failing input

```text
ushr v0.8b, v1.8b, #20     // arrangement "8b", shift = 20  (> 2*esize = 16)
```

## Expected vs. actual

* **Expected:** `Err` (the shift is not a valid USHR shift amount).
* **Actual (debug build):** the process **panics**:

> `thread '…' panicked at src/backend/arm/assembler/encoder/neon.rs:1192:25:
> attempt to subtract with overflow`

The witness covers `shift ∈ {2*esize+1, 4*esize}` for every supported
arrangement; all panic in debug builds.

## Impact

Any malformed `#shift` of roughly `≥ 17` (bytes) / `≥ 33` (half) / `≥ 65`
(word) / `≥ 129` (double) aborts the entire assembler process instead of
returning `Err`. This is a robustness/availability defect: malformed user
input must never crash the encoder.

## Suggested fix

Use wrapping subtraction and reject out-of-range shifts before encoding:

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
