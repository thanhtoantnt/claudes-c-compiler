# Bug Report: `encode_madd` silently accepts SP/WSP as accumulator operand

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_madd`

## Summary

`encode_madd` resolves the accumulator operand through the shared generic register parser, which maps `sp`/`wsp` to register number 31. In the data-processing (3-source) encoding class, register 31 is the zero register, not SP, so `sp` is not a valid accumulator operand for MADD. The encoder accepts it and silently re-encodes it as XZR/WZR.

## Reproduction

Failing property: `madd_rejects_sp_operand`

Minimal input:

```text
madd x0, x1, x2, sp
```

Actual behavior: returns `Ok(Word(_))` and encodes it as if the accumulator were `xzr`.

## Impact

A source typo that names `sp` instead of `xzr` is accepted without diagnostic, changing the meaning of the instruction.

## Suggested fix

Reject `sp`/`wsp` in the accumulator position for MADD/MSUB-style data-processing (3-source) encoders before converting the token to a register number.
