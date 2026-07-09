# Bug Report: `encode_neon_tbx` silently accepts non-byte arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** Medium

## Summary

TBX is defined only for `.8b`/`.16b`. The encoder treats every other arrangement as `Q=0` and emits a bogus `.8b`-shaped word.

## Root Cause

```rust
let q: u32 = if arr_d == "16b" { 1 } else { 0 };  // all non-16b -> Q=0
```

## Reproduction

**Input:** `tbx v0.4h, {v1.16b}, v2.4h`

**Expected:** `Err` — only .8b/.16b allowed

**Actual:** `Ok(Word(0x0E021020))` — silent .8b reinterpretation

## Impact

Typo'd arrangements silently mis-assemble to wrong element width.

## Suggested Fix

```rust
match arr_d.as_str() {
    "8b" => 0, "16b" => 1,
    other => return Err(format!("tbx: unsupported arrangement '{}'", other)),
}
```

## Regression Property

Failing property: `tbx_rejects_non_byte_arrangements`

```rust
prop_assert!(encode_neon_tbx(&[va(0,"4h"), list, va(2,"4h")]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/246
