# Bug Report: `encode_negs` silently re-encodes ROR shift as LSL

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_negs`

## Summary

`encode_negs` accepts `ror` in the optional shift operand and maps it through the default match arm to shift kind `0b00`, i.e. LSL. Add/sub shifted-register encodings permit only LSL/LSR/ASR; ROR is invalid and should be rejected.

## Reproduction

Failing property: `negs_rejects_ror_shift`

Minimal input:

```text
negs w0, w0, ror #0
```

Actual result: `Ok(Word(_))`, encoded as if the shift were `lsl #0`.

## Impact

Invalid source code is silently accepted and assembled as a different instruction than written.

## Suggested fix

Reject unknown shift kinds instead of using a default LSL arm:

```rust
let shift_type = match kind.as_str() {
    "lsl" => 0b00,
    "lsr" => 0b01,
    "asr" => 0b10,
    other => return Err(format!("negs invalid shift kind: {}", other)),
};
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/81
