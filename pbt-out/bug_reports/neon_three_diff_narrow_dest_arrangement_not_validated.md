# NEON `encode_neon_three_diff_narrow` — destination arrangement not validated (silently accepted)

## Function
`src/backend/arm/assembler/encoder/neon.rs::encode_neon_three_diff_narrow`

Encodes the AArch64 NEON "Advanced SIMD three different" **narrowing** family
(ADDHN/RADDHN/SUBHN/RSUBHN and their `2` variants).

## Symptom
The encoder derives the entire 32-bit word from the **source** register's
arrangement (`arr_n`, operand 1) plus `is_high`. The destination register
(operand 0) is read for its register number only:

```rust
let (rd, _) = get_neon_reg(operands, 0)?;   // arrangement discarded, never validated
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let (rm, _) = get_neon_reg(operands, 2)?;
let size = match arr_n.as_str() { "8h" => 0b00u32, "4s" => 0b01, "2d" => 0b10, ... };
```

Consequences, all empirically confirmed (`probe` test):
- A **mismatched** destination arrangement is silently accepted and produces the
  SAME word as the correct narrow destination.
- A **bare** `Operand::Reg("v0")` with no arrangement at all is also accepted.

The reference AArch64 assembler (`clang --target=aarch64`) rejects both with
`error: invalid operand for instruction`.

## Reproduction (property test, `#[ignore]`d)
`src/backend/arm/assembler/encoder/neon_three_diff_narrow_pbt.rs`
→ `narrow_rejects_inconsistent_destination_arrangement`

Minimal failing input recorded by proptest:
```
bad_dest = "4s"     // addhn v0.4s, v1.8h, v2.8h  — dest should be .8b
=> Ok(Word(0x0E224020))   // identical to the valid addhn v0.8b, v1.8h, v2.8h
```

Empirical witness (correct vs. mismatched dest produce the same word):
```
valid   addhn v0.8b, v1.8h, v2.8h  => 0x0E224020   (== LLVM golden word)
invalid addhn v0.4s, v1.8h, v2.8h  => Ok(0x0E224020)  (silently accepted)
invalid addhn v0,    v1.8h, v2.8h  => Ok(0x0E224020)  (bare Reg, silently accepted)
```

Run with:
```
cargo test --lib narrow_rejects_inconsistent_destination_arrangement -- --ignored
```

## Impact
Low today (the function's only callers in `encoder/mod.rs` dispatch fixed,
correct operands), so the defect is latent — but if a caller ever hands a
mismatched or arrangement-less destination, the encoder emits the right bytes
for the *wrong-looking* instruction with no error, defeating assembler-level
diagnostics. This is the same "minimal validation, trust the parser" posture as
the sibling widening encoder `encode_neon_three_diff`.

## Why this is classified a bug (not a documented caveat)
No repo docstring, spec note, or existing test states that the destination
arrangement is *intentionally* ignored/permitted. Per bug-by-default, an
unvalidated input that a reference assembler rejects is a defect unless cited
as intentional — no such citation exists.

## Suggested fix
After computing `size`/`Q` from the source, assert the destination arrangement
is the expected narrow type for the family (e.g. `.8b`/`.16b` for `.8h` source,
`.4h`/`.8h` for `.4s` source, `.2s`/`.4s` for `.2d` source), and require a
non-empty arrangement; return `Err` otherwise.
