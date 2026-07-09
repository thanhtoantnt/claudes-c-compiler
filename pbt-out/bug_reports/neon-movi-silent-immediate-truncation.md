# Bug Report — `encode_neon_movi` silently truncates out-of-range immediates

**File:** `src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_movi`
**Severity:** Medium (incorrect codegen, silent — no diagnostic emitted)

## Summary

For the byte / 32-bit / 16-bit element forms (`.8b`, `.16b`, `.2s`, `.4s`, `.4h`,
`.8h`), `encode_neon_movi` masks the immediate with `imm as u32 & 0xFF` and emits
a valid word for any input, instead of rejecting immediates that fall outside the
8-bit field the instruction actually encodes.

Per the A64 ISA, the MOVI immediate for these forms is an 8-bit value
(`imm8`, range `0..=255`). Real assemblers (GAS / LLVM-MC) reject out-of-range
values:

```
MOVI V0.8B, #256
error: immediate must be an integer in range [0, 255]
```

The encoder accepts it and silently encodes `#256` as `#0` (because `256 & 0xFF == 0`).
This is also inconsistent with the `.2d` branch in the **same function**, which
*does* validate strictly (each byte must be `0x00`/`0xFF`, otherwise `Err`) — so the
author clearly knows how to reject invalid immediates, but omitted the check on the
other four branches.

## Root cause

```rust
// .8b / .16b branch (same pattern in .2s/.4s and .4h/.8h):
let imm8 = imm as u32 & 0xFF;   // <-- masks silently; no range check / no Err
```

Negative immediates are mishandled too: `imm = -1i64` becomes `0xFFFFFFFFu32`, then
`& 0xFF == 0xFF`, so `MOVI V0.8B, #-1` silently encodes as `MOVI V0.8B, #0xFF`.

`.2d` is correct and rejects e.g. `imm = 1` (byte `0x01` is neither `0x00` nor
`0xFF`).

## Reproduction

Property `out_of_range_immediate_must_be_rejected` in
`mod neon_movi_props` (`neon.rs`) fails on the minimal case:

```
arr = "8b", imm = 256
MOVI "8b" #256 is out of 8-bit range and must be rejected,
got Ok(Word(251716608))
```

`251716608 == 0x0F00E400`, which is exactly `MOVI V0.8B, #0`.

```bash
cargo test --lib neon_movi_props::out_of_range_immediate_must_be_rejected
```

## Suggested fix

Validate the immediate range before masking, mirroring the `.2d` branch:

```rust
// for .8b/.16b/.2s/.4s/.4h/.8h:
if !(0..=255).contains(&imm) {
    return Err(format!("movi {}: immediate {} out of range [0,255]", arr_d, imm));
}
let imm8 = imm as u32 & 0xFF;
```

## Test coverage added

8 property-based tests in `mod neon_movi_props` (`proptest`):

| Test | Oracle | Result |
|------|--------|--------|
| `byte_form_field_layout` | reference layout (every field of the word) | PASS |
| `field_independence` | algebraic (Rd/imm bits don't cross-contaminate) | PASS |
| `cmode_per_form` | cmode selection per arrangement | PASS |
| `shift_selects_cmode` | LSL shift → cmode 0000/0010/0100/0110 | PASS |
| `unsupported_arrangement_rejected` | negative contract | PASS |
| `unsupported_shift_rejected` | negative contract | PASS |
| `d2_form_strict_byte_validation` | reference/negative (.2d is strict) | PASS |
| `out_of_range_immediate_must_be_rejected` | negative contract | **FAIL** |

No cross-assembler (llvm-mc / aarch64 `as`) was available in this environment for
differential validation, so the reference oracle is derived directly from the
AdvSIMD modified-immediate bit layout (independently re-derived, not copied from
the implementation).
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/181
