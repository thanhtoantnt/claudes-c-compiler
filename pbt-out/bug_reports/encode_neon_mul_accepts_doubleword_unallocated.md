# Bug: `encode_neon_mul` accepts `.1d`/`.2d` arrangements (UNALLOCATED)

## Summary
Vector MUL is architecturally defined only for `size != 0b11` (`.8b`/`.16b`/`.4h`/`.8h`/`.2s`/`.4s`). The encoder silently accepts `.1d` and `.2d` (size=0b11), emitting instruction words that are UNALLOCATED on hardware and will trigger SIGILL.

## Witness
```
cargo test -- --ignored rejects_unallocated_doubleword
```
Fails with shrunk counterexample showing `Ok(Word(...))` for `.2d` input.

## Root cause
Same as `encode_neon_mla`/`encode_neon_mls` — the arrangement-to-size mapping does not reject `size == 0b11`.

## Severity
MEDIUM — emits UNALLOCATED encoding; undefined behavior on hardware.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/296
