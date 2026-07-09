# Bug Report: `encode_neon_rbit` silently accepts mismatched source arrangement

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_rbit`
**Severity:** Medium

## Summary

`encode_neon_rbit` validates the **destination** arrangement (`arr_d`) but
**discards the source arrangement** (`arr_n`). RBIT (vector) is specified as
`RBIT <Vd>.<T>, <Vn>.<T>` where **both** operands must share the same
arrangement `T`, and `T` is restricted to `8B` or `16B` only (ARM DDI 0487,
*RBIT (vector)*). The encoder therefore silently accepts illegal pairings such
as `rbit v0.16b, v1.4s` and emits a well-formed RBIT word identical to the
legal `rbit v0.16b, v1.16b`, instead of returning an assembler error.

This is a genuine **silent mis-assembly**: the malformed input is accepted and
the user gets no diagnostic. A real assembler (GNU `as` / LLVM `llvm-mc`) would
reject `rbit v0.16b, v1.4s` with an *operand size mismatch* error.

## Root Cause

In `encode_neon_rbit`, the source arrangement is read and immediately thrown
away:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;   // <-- arr_n discarded

// Only .8b and .16b arrangements are valid for NEON RBIT
if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("neon rbit: unsupported arrangement .{}, expected .8b or .16b", arr_d));
}
```

Only `arr_d` is checked. Because `Rn` occupies a fixed field and the encoding
does not otherwise depend on the source arrangement, a mismatched source
produces a bit-for-bit identical word to the corrected input, so the bug is
invisible without an explicit check.

The input is fully reachable through the normal parse path: the parser turns
any token of the form `vN.Ts` (with `Ts ∈ {8b,16b,4h,8h,2s,4s,1d,2d,…}`) into
`Operand::RegArrangement { reg, arrangement }` regardless of operand position
(`parser.rs`, `RegArrangement` construction).

## Reproduction

Concrete inputs, all of which the encoder accepts today:

| Input                     | Actual result          | Expected |
|---------------------------|------------------------|----------|
| `rbit v0.16b, v1.4s`      | `Ok(0x6E605820)`       | `Err`    |
| `rbit v0.16b, v1.2d`      | `Ok(0x6E605820)`       | `Err`    |
| `rbit v0.8b,  v1.4h`      | `Ok(0x2E605820)`       | `Err`    |

`rbit v0.16b, v1.4s` is emitted as `0x6E605820` — the **exact** word the legal
`rbit v0.16b, v1.16b` produces. Run the `#[ignore]`d test
`rbit_mismatch_reproduction` to see this directly.

## Impact

- **Silent mis-assembly.** A user who mistypes the source arrangement (a common
  typo, e.g. copy-pasting `.4s` from a neighbouring instruction) gets a valid
  binary that performs the *destination's* RBIT on the source register with no
  warning. The resulting program executes but does not match the source text.
- **Inconsistent validation surface.** The encoder *does* validate the
  destination arrangement, which sets the expectation that bad arrangements are
  rejected; the unchecked source is a latent trap.
- **No defensive layer above.** Nothing between the parser and the encoder
  cross-checks the two arrangements, so the only place this can be caught is
  here.

Severity is **Medium**: it does not corrupt legal programs (those still encode
correctly, as the golden table confirms), but it silently rewrites illegal ones
into legal, different instructions.

## Suggested Fix

Validate that the source arrangement matches the destination (and is itself a
byte arrangement), mirroring the existing destination check. The minimal fix:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;   // <-- capture arr_n

if arr_d != "8b" && arr_d != "16b" {
    return Err(format!("neon rbit: unsupported arrangement .{}, expected .8b or .16b", arr_d));
}
if arr_n != arr_d {
    return Err(format!(
        "neon rbit: arrangement mismatch (Vd.{arr_d} vs Vn.{arr_n}); both operands must be .8b or both .16b"
    ));
}
```

## Regression Property

Failing property: `rbit_rejects_mismatched_source_arrangement`

```rust
#[test]
#[ignore]
fn rbit_rejects_mismatched_source_arrangement(
    src_arr in prop_oneof![
        Just("4h"), Just("8h"), Just("2s"), Just("4s"), Just("1d"), Just("2d")
    ],
    rd in 0u32..=31,
    rn in 0u32..=31,
    is_q in any::<bool>(),
) {
    let dst_arr = if is_q { "16b" } else { "8b" };
    let ops = vec![va(rd, dst_arr), va(rn, src_arr)];
    prop_assert!(
        encode_neon_rbit(&ops).is_err(),
        "rbit v{rd}.{dst_arr}, v{rn}.{src_arr} must be rejected (arrangement mismatch); got Ok",
    );
}
```

After applying the fix, remove the `#[ignore]` and the property holds for all
mismatched sources. (A companion finding, `rbit_rejects_non_vector_register_class`,
documents that GPR-class names like `x0.16b` are also accepted in SIMD operand
position via `parse_reg_num`; it is filed separately as it stems from the shared
`parse_reg_num`/`get_neon_reg` helper rather than this function's own logic.)
