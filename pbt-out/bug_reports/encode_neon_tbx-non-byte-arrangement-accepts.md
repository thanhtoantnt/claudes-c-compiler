# Bug — `encode_neon_tbx` silently accepts non-byte destination arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** medium (silent mis-assembly of the Q bit)
**Witness test:** `tbx_rejects_non_byte_arrangements`
(`#[ignore]`d in `src/backend/arm/assembler/encoder/neon_tbx_pbt.rs`)

## Defect

```rust
let q: u32 = if arr_d == "16b" { 1 } else { 0 };
```

`TBX` is defined **only** for `.8b`/`.16b` destinations. The encoder treats
*every* other arrangement (`.4h`, `.8h`, `.2s`, `.4s`, `.1d`, `.2d`, …) as
`Q=0` and emits a bogus `.8b`-shaped word instead of `Err`. `get_neon_reg`
accepts these arrangements, so they are not caught upstream.

## Minimal failing input

`rd=0, arr_d="4h"` (table `{v1.16b}`, index `v2.4h`)

## Expected vs. actual

- **Expected:** `Err` (`tbx` is defined only for `.8b`/`.16b`).
- **Actual:** `Ok(Word(0x0E021020))` — a `.8b` (Q=0) TBX word, even though
  the source named a halfword arrangement.

## Impact

A typo'd or unsupported arrangement is silently reinterpreted as `.8b`,
producing an instruction whose element width does not match the source
intent — a quiet correctness bug.

## Fix

Validate the arrangement before deriving `Q`:

```rust
match arr_d.as_str() {
    "8b"  => q = 0,
    "16b" => q = 1,
    other => return Err(format!("tbx: unsupported arrangement '{}', only .8b/.16b", other)),
}
```

## Reproduce

```bash
cargo test --lib neon_tbx_pbt -- --ignored tbx_rejects_non_byte_arrangements
```
