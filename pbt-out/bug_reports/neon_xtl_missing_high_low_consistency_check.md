# Bug: `encode_neon_xtl` does not validate `is_high` against the source arrangement half

**File:** `src/backend/arm/assembler/encoder/neon.rs`
**Function:** `encode_neon_xtl(operands, u_bit, is_high)`
**Witness test:** `neon_xtl_pbt::witness_high_low_consistency_unchecked` (`#[ignore]`d)

## Summary

`encode_neon_xtl` (the UXTL/SXTL encoder, an alias for USHLL/SSHLL with
`shift == 0`) accepts **any** combination of `is_high` and source arrangement
without cross-checking that the source half is consistent with the low (`uxtl`)
vs. high (`uxtl2`) variant. A real assembler (GNU `as`, LLVM `llvm-mc`) rejects
such operands; this encoder silently emits a valid-but-textually-wrong word.

## Reproduction

```rust
use ccc::backend::arm::assembler::encoder::encode_neon_xtl;
use ccc::backend::arm::assembler::parser::Operand;

fn v(reg: u32, arr: &str) -> Operand {
    Operand::RegArrangement { reg: format!("v{}", reg), arrangement: arr.into() }
}

// `uxtl v0.8h, v1.16b` — low variant, but a WIDE-half .16b source.
let res = encode_neon_xtl(&[v(0, "8h"), v(1, "16b")], /*u_bit=*/ 0, /*is_high=*/ false);
assert!(res.is_ok()); // <-- BUG: should be Err
// returns Ok(Word(0x0F_0A_08_50)) = 252_224_544
```

Run against the current build:

```
$ cargo test --lib neon_xtl_pbt::witness_high_low_consistency_unchecked -- --ignored
... panicked: uxtl (is_high=false) with a .16b wide-half source must be rejected,
    got Ok(Word(252224544))
```

## Root cause

```rust
let immh = match arr_n.as_str() {
    "8b" | "16b" => 0b0001u32,   // width only — half not distinguished
    "4h" | "8h"  => 0b0010,
    "2s" | "4s"  => 0b0100,
    _ => return Err(...),
};
let q = if is_high { 1u32 } else { 0 };   // set purely from is_high
```

The arrangement match conflates the two halves of each width (`.8b`/`.16b`,
`.4h`/`.8h`, `.2s`/`.4s`). `Q` is taken only from `is_high`. Nothing enforces
the ARM ARM alias constraint that:

* the **low** variant (`uxtl`/`sxtl`, `Q == 0`) requires a **narrow-half** source
  (`.8b`, `.4h`, `.2s`), and
* the **high** variant (`uxtl2`/`sxtl2`, `Q == 1`) requires a **wide-half**
  source (`.16b`, `.8h`, `.4s`).

## Impact

* **Silent mis-encoding.** A typo'd or malformed instruction like
  `uxtl v0.8h, v1.16b` assembles to a *valid* USHLL word (`immh=0001`, `Q=0`,
  i.e. an 8→16-bit widen of the low 8 bytes) instead of erroring. The emitted
  instruction does not match the textual source.
* Inconsistent input that every conforming AArch64 assembler rejects is accepted
  without a diagnostic, defeating the encoder's otherwise-complete validation
  (registers v0–v31 and the arrangement whitelist *are* checked correctly).

## Suggested fix

After computing `immh`, validate the half against `is_high`, e.g.:

```rust
let is_wide_half = matches!(arr_n.as_str(), "16b" | "8h" | "4s");
if is_high != is_wide_half {
    return Err(format!(
        "uxtl/sxtl: source arrangement {} is the {} half but is_high={}",
        arr_n, if is_wide_half { "wide" } else { "narrow" }, is_high
    ));
}
```

## Validation status of the rest of the function (verified, no bug)

* Register numbers `v0..=v31` ARE validated via `parse_reg_num` (returns `None`
  for `v32+`); out-of-range registers are rejected, not masked into 5 bits.
* Source arrangements ARE whitelisted; unsupported ones (`.2d`, `.1d`, `.16h`,
  garbage, empty, …) return `Err`.
* The encoded bit layout matches the ARM ARM `0 Q U 011110 immh 000 101001 Rn Rd`
  form exactly (verified by `prop_xtl_encoding_matches_reference` and
  `prop_field_isolation`).

## Test artifacts

* `src/backend/arm/assembler/encoder/neon_xtl_pbt.rs` — 5 proptest properties +
  2 negative-contract tests (all pass on default `cargo test`), plus the
  `#[ignore]`d witness above.
* Default `cargo test` stays green: 7 passed, 1 ignored.
