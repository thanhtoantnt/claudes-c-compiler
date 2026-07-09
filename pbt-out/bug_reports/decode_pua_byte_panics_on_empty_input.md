# Bug Report: `decode_pua_byte` panics on empty input / out-of-range `pos`

**Target:** `src/common/encoding.rs` → `decode_pua_byte`
**Severity:** Low

## Summary

`decode_pua_byte` indexes `input[pos]` unconditionally in fallback branch. Empty slice or `pos >= len` triggers panic without guard.

## Root Cause

```rust
pub fn decode_pua_byte(input: &[u8], pos: usize) -> (u8, usize) {
    if pos + 2 < input.len() && input[pos] == 0xEE { ... }
    (input[pos], 1)   // panics if pos >= input.len()
}
```

`pos < input.len()` is only implicit contract; caller `decode_all_pua_bytes` honours it but public function has no guard.

## Reproduction

**Input:** `decode_pua_byte(&[], 0)`

**Expected:** Either documented precondition or non-panicking return (e.g. `Option`)

**Actual:** Panic: index out of bounds

**Minimal failing input:** bytes = [], pos = 0

## Impact

Any caller passing empty slice or `pos >= len` triggers panic. In-tree caller safe, but public API unguarded.

## Suggested Fix

Add guard and return `Option`:

```rust
pub fn decode_pua_byte(input: &[u8], pos: usize) -> Option<(u8, usize)> {
    if pos >= input.len() {
        return None;
    }
    if pos + 2 < input.len() && input[pos] == 0xEE { ... }
    Some((input[pos], 1))
}
```

## Regression Property

Failing property: `decode_does_not_panic_at_end_position`

```rust
prop_assert!(decode_pua_byte(&[], 0).is_none());
prop_assert!(decode_pua_byte(&[0x00], 1).is_none());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/161