# Bug: `encode_neon_movi` silently accepts invalid shift kinds (lsr/asr/ror)

## Summary
Only LSL and MSL are valid shift operators for MOVI. The encoder's else-branch falls through to `cmode = 0b0000` for any unrecognized shift kind (`lsr`/`asr`/`ror`), returning `Ok` instead of `Err`. The instruction silently becomes the unshifted form.

## Witness
```
cargo test -- --ignored movi_silently_accepts_invalid_shift_kind
```
Fails: `lsr is not a valid MOVI shift; expected Err but got Ok(0x...)`.

## Root cause
No validation that `kind` is `"lsl"` or `"msl"` — anything else falls through the else arm.

## Severity
MEDIUM — silent misencoding; invalid assembly is accepted and produces wrong output.
