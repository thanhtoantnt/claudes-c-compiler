# Bug — PUA range ambiguity silently corrupts literal U+E080..U+E0FF bytes

**File:** `src/common/encoding.rs` — `decode_pua_byte` / `encode_non_utf8`
**Severity:** Low–Medium (silent data corruption)

## Minimal input

Raw byte vector: `[0xEE, 0x82, 0x80]` (valid UTF-8 for code point `U+E080`).

## Expected

`decode_all_pua_bytes(&bytes_to_string(vec![0xEE, 0x82, 0x80])) == [0xEE, 0x82, 0x80]`.

## Actual

Captured by failing proptest `raw_pua_range_utf8_does_not_roundtrip`
(minimal failing input: `cp = 0xE080`, prefix/suffix empty):

```
assertion `left == right` failed
  left:  [128]            (= 0x80)
  right: [238, 130, 128]  (= EE 82 80, the original bytes)
```

The encoder passes the valid-UTF-8 triple through unchanged
(`input == bytes`), but `decode_pua_byte(.., 0)` returns `(0x80, 3)`,
collapsing 3 bytes into 1.

## Root cause

`encode_non_utf8` PUA-encodes only bytes that fail UTF-8 validation; any byte run
that already forms valid UTF-8 — including the `EE 82/83 80–BF` triples that are the
PUA range's own UTF-8 form — is preserved verbatim. `decode_pua_byte` then
pattern-matches those preserved triples as PUA encodings. The two directions
disagree on the meaning of literal `EE 82/83 80–BF`.

## Impact

Any content in a C string/char literal containing these triples (binary/legacy
data) is silently mangled, with no error or warning. The now-unused test helper
`contains_literal_pua_utf8` appears designed to detect this, suggesting prior
awareness.

## Fix

In `encode_non_utf8`, also re-encode valid-UTF-8 code points in `U+E080..U+E0FF` so
the encoder never emits a literal PUA-range triple for a passthrough character
(escape the PUA range, e.g. map an original `U+E080` to a two-PUA-char sequence).
