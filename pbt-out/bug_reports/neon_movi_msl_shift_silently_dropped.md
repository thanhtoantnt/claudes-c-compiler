# Bug: `encode_neon_movi` silently drops MSL shift on `.2s`/`.4s`

## Summary
MSL (Modified Shift Left) is a valid shift for the 32-bit element MOVI form: `msl #8` → cmode=1100, `msl #16` → cmode=1101. The encoder's `.2s`/`.4s` branch only matches `kind == "lsl"`, so an MSL operand falls through to `cmode = 0000` and the shift is silently dropped.

## Witness
```
cargo test -- --ignored movi_drops_msl_shift_on_32bit
```
Fails: `cmode dropped to 0b0000 (expected 0b1100)`.

## Root cause
`neon.rs` MOVI `.2s`/`.4s` branch checks `kind == "lsl"` but does not handle `kind == "msl"`.

## Severity
MEDIUM — silent misencoding; the instruction executes as the unshifted form.
