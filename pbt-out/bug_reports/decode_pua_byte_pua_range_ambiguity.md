# Bug Report: PUA range ambiguity silently corrupts literal U+E080..U+E0FF bytes

**Target:** `src/common/encoding.rs` → `decode_pua_byte` / `encode_non_utf8`
**Severity:** Medium

## Summary

`encode_non_utf8` PUA-encodes only bytes that fail UTF-8 validation; valid UTF-8 triples `EE 82/83 80–BF` (the PUA range's own UTF-8 form) are preserved. `decode_pua_byte` then pattern-matches those preserved triples as PUA encodings. Roundtrip fails.

## Root Cause

```rust
// encode_non_utf8
if !std::str::from_utf8(input).is_ok() {
    // PUA-encode
} else {
    // Preserve verbatim — including PUA-range UTF-8!
}

// decode_pua_byte
if pos + 2 < input.len() && input[pos] == 0xEE { ... }  // Matches preserved triples
```

## Reproduction

**Input:** Raw bytes `[0xEE, 0x82, 0x80]` (valid UTF-8 for U+E080)

**Expected:** Roundtrip returns original `[0xEE, 0x82, 0x80]`

**Actual:** Returns `[0x80]` — collapsed 3 bytes into 1

**Minimal failing input:** cp = 0xE080, prefix/suffix empty

## Impact

Binary/legacy data containing PUA-range UTF-8 triples is silently mangled with no error or warning. Test helper `contains_literal_pua_utf8` suggests prior awareness.

## Suggested Fix

Re-encode valid UTF-8 code points in `U+E080..U+E0FF` so encoder never emits literal PUA-range triple:

```rust
// In encode_non_utf8, also escape PUA-range UTF-8
let (cp, _) = decode_utf8_char(input, i)?;
if (0xE080..=0xE0FF).contains(&cp) {
    // Re-encode as two-PUA-char sequence
}
```

## Regression Property

Failing property: `raw_pua_range_utf8_does_not_roundtrip`

```rust
prop_assert_eq!(decode_all_pua_bytes(&bytes_to_string(vec![0xEE, 0x82, 0x80])),
                vec![0xEE, 0x82, 0x80]);
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/160