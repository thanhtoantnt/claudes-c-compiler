# Bug — `encode_neon_float_elem` silently truncates out-of-range lane index

**File:** `src/backend/arm/assembler/encoder/neon.rs` — function `encode_neon_float_elem`
**Tests:** `src/backend/arm/assembler/encoder/neon_float_elem_pbt.rs`
**Oracle:** `llvm-mc-18 --triple=aarch64 --assemble --show-encoding` + ARMv8 ARM.
**Severity:** medium — a wrong lane is selected with no error.

## Defect

`llvm-mc-18` rejects `fmul v0.4s,v0.4s,v0.s[4]` with *"vector lane must be an
integer in range [0, 3]"* and `fmul v0.2d,v0.2d,v0.d[2]` with *"range [0, 1]"*.
The encoder performs **no** range check and masks the index instead:

```rust
let (h, l, m_bit) = if sz == 0 {
    ((index >> 1) & 1, index & 1, (rm >> 4) & 1)   // .s: only low 2 bits used
} else {
    (index & 1, 0u32, (rm >> 4) & 1)               // .d: only low 1 bit used
};
```

So `.s[4]` aliases `.s[0]`, `.s[5]` aliases `.s[1]`, `.d[2]` aliases `.d[0]`,
etc. — a wrong lane is selected with no error. The sibling integer by-element
encoder (`encode_neon_elem_long`) *does* validate index ranges, so this is an
inconsistency, not an intentional design.

## Valid ranges

| element size | `sz` | max lane index |
|--------------|------|----------------|
| `.s`         | 0    | 3              |
| `.d`         | 1    | 1              |

## PBT witness

Surfaced by the FAILING proptest property `prop_rejects_out_of_range_lane_index` (#[ignore]d).

- **reproduce=** `cargo test --lib neon_float_elem_pbt::prop_rejects_out_of_range_lane_index -- --ignored`
- **shrunk counterexample (proptest "minimal failing input"):** `bad = (0, 4)`
  → `sz = 0` (`.s`), `index = 4` (max is 3), i.e. lane `v0.s[4]`.
- **Falsifiable:** encoder returns `Ok(Word(1325436928))` instead of `Err`; the
  property expects rejection because `4 > max(3)` and no spec defines
  wrapping/truncation as intentional.

Concrete observed failure:
```
prop_rejects_out_of_range_lane_index: Test failed: out-of-range lane [4] for .s
must be rejected, not silently truncated; got Ok(Word(1325436928))
```

## Suggested fix

```rust
let max = if sz == 0 { 3u32 } else { 1u32 };
if index > max {
    return Err(format!("float by-element: lane index {index} out of range [0, {max}]"));
}
```
