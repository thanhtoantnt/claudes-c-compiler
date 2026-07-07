# Bug Report — `parse_reg_num` over-accepts malformed register names

**File:** `src/backend/arm/assembler/encoder/mod.rs`
**Function:** `parse_reg_num(name: &str) -> Option<u32>` (line ~131)

## Summary

`parse_reg_num` parses the numeric suffix of a register name with
`name[1..].parse::<u32>()`. Rust's integer `FromStr` accepts an optional
leading `+` and arbitrary leading zeros, so several inputs that are **not**
valid AArch64 register spellings are silently accepted and mapped to a real
register number instead of being rejected (`None`).

## Reproduction

```
parse_reg_num("x+5")  -> Some(5)   // expected: None — "x+5" is not a register
parse_reg_num("x+0")  -> Some(0)   // expected: None
parse_reg_num("x007") -> Some(7)   // expected: None (or Some(7) only if leading
                                   //   zeros are deemed intentional — they are not)
parse_reg_num("w+31") -> Some(31)  // expected: None
```

Verified empirically:
```
"+5".parse::<u32>()  -> Ok(5)
"007".parse::<u32>() -> Ok(7)
```

## Impact

* **Severity: low.** The built-in assembler is normally fed by the project's
  own codegen, which never emits `x+5`/`x007`. In practice no mis-encoding
  occurs today.
* **Conceptual risk:** the documented contract is "Parse a register name to
  its 5-bit encoding number (0–30, 31 for sp/zr)". The current implementation
  does not enforce that the suffix is a *canonical decimal integer*, so a
  hand-written or attacker-controlled `.s` input could name a register in a
  non-standard way and still assemble. This weakens the negative contract
  (tested by `out_of_range_reg_rejected` / `non_numeric_suffix_rejected`),
  even though the *value range* (`<= 31`) is correctly enforced — i.e. there
  is **no silent truncation**, only over-acceptance of malformed spellings.

## Positive coverage (working correctly)

The following invariants are **not** affected and are covered by the new
property suite in `parse_reg_num_props`:

* Numeric round-trip `<prefix><n>` → `Some(n)` for `n` in `0..=31` across all
  eight bank prefixes (`x/w/d/s/q/v/h/b`).
* Decoded number is independent of the bank prefix (shared 5-bit field).
* Out-of-range numbers (`n > 31`) → `None` (no masking/truncation).
* Case-insensitivity (upper/lower yield same result).
* Special registers: `sp/wsp/xzr/wzr` → 31, `lr` → 30.

## Suggested fix

Reject any suffix that is not a plain run of ASCII digits with no sign and no
redundant leading zero (unless the suffix is exactly `"0"`):

```rust
let suffix = &name[prefix.len_utf8()..];
if !suffix.bytes().all(|b| b.is_ascii_digit())
    || (suffix.starts_with('0') && suffix.len() > 1)
{
    return None;
}
let num: u32 = suffix.parse().ok()?;
if num <= 31 { Some(num) } else { None }
```

(Note: `prefix` must be obtained via `chars().next()` and sliced with
`len_utf8()` to remain byte-aligned; the current `name[1..]` is only safe
because every matched prefix is ASCII.)

## Test artifacts

Property suite added inline at the bottom of
`src/backend/arm/assembler/encoder/mod.rs` (module `parse_reg_num_props`,
7 tests). Run with:

```
cargo test --lib parse_reg_num_props
```
