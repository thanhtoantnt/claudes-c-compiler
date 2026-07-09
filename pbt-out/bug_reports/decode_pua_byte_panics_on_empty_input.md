# Bug — `decode_pua_byte` panics on empty input / out-of-range `pos`

**File:** `src/common/encoding.rs` — `decode_pua_byte`
**Severity:** Low (robustness; implicit contract)

## Minimal input

`decode_pua_byte(&[], 0)` — empty slice, position 0.

## Expected

Either a documented precondition (`pos < input.len()`) or a non-panicking return.

## Actual

Index-out-of-bounds panic, captured by failing proptest
`decode_does_not_panic_at_end_position` (minimal failing input:
`bytes = []`, `pos = 0`):

```
decode_pua_byte panicked at pos=0 of len=0
```

The fallback branch indexes `input[pos]` unconditionally:

```rust
pub fn decode_pua_byte(input: &[u8], pos: usize) -> (u8, usize) {
    if pos + 2 < input.len() && input[pos] == 0xEE { ... }
    (input[pos], 1)   // panics if pos >= input.len()
}
```

## Impact

`pos < input.len()` is only an implicit contract. The in-tree caller
`decode_all_pua_bytes` honours it (`while pos < input.len()`), but the public
function has no guard, so any caller that passes an empty slice or a `pos >= len`
triggers a panic.

## Fix

Add `pos < input.len()` to the guard and decide a fallback (e.g. return
`Option<(u8, usize)>`, or document the precondition explicitly in the doc comment).
