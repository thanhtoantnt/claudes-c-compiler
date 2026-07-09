# Bug — `encode_neon_shll`: negative immediate PANICS (`attempt to add with overflow`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs`, `pub(crate) fn encode_neon_shll` (line ~1480)
**Test:** `src/backend/arm/assembler/encoder/neon_shll_pbt.rs` — `prop_shll_rejects_negative_shift` (failing, shrunk PBT property) + deterministic witness `negative_shift_produces_unallocated`

## Summary

The SHLL/SHLL2 encoder reads the shift immediate via `get_imm` (which returns
`i64`) and casts it `as u32` with no sign check. A negative immediate such as
`#-1` becomes `0xFFFFFFFF`, and the subsequent `base_val + shift` (`u32 + u32`)
**overflows** — panicking in debug builds and producing `immh = 0000`
(UNALLOCATED, corrupt word) in release builds. The function's public signature
is `Result<EncodeResult, String>`, so a malformed-but-parseable operand should
return `Err`, not abort the process.

## Minimal failing input (shrunk PBT witness)

`prop_shll_rejects_negative_shift` shrunk to: `rd=0, rn=0, arr_n="8b", shift=-2, u_bit=0, is_high=false`.

Equivalent assembly: `sshll v0.8h, v0.8b, #-2`.

reproduce: `cargo test --lib neon_shll_pbt::prop_shll_rejects_negative_shift`

## Expected vs actual

- **Expected:** `Err(...)` — a negative shift is illegal.
- **Actual (debug):**
  ```
  thread '...' panicked at src/backend/arm/assembler/encoder/neon.rs:1480:17:
  attempt to add with overflow
  ```
- **Actual (release):** `Ok(Word(...))` with `immh = 0000` (UNALLOCATED).

## Impact

Process abort reachable from any caller that forwards a parsed
`Operand::Imm(-1)` (or any negative). A single crafted `#-1` in assembly input
turns the `Result` API into a crash (debug) or an unallocated instruction word
(release). This is the more severe of the two SHLL findings because it crosses
the `Result`-returning API boundary with a panic.

## Fix

Reject negatives explicitly before the cast, alongside the range check for
Finding 1 (`encode_neon_shll-over-range-shift-silent-misencode.md`):

```rust
let shift_i = get_imm(operands, 2)?;
if shift_i < 0 {
    return Err(format!("sshll/ushll: shift {shift_i} must be non-negative"));
}
let shift = shift_i as u32;
```

## Reproduce

```
cargo test --lib neon_shll_pbt::negative_shift_produces_unallocated
```
