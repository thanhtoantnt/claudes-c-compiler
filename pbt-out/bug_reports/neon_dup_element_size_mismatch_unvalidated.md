# Bug — `encode_neon_dup` element form accepts arrangement / element-size mismatch

**Function:** `encode_neon_dup`, element-form branch
(`DUP Vd.T, Vn.Ts[index]`)
**File:** `src/backend/arm/assembler/encoder/neon.rs`
**Witness:** `prop_rejects_arrangement_element_size_mismatch` in
`src/backend/arm/assembler/encoder/neon_dup_pbt.rs` (marked `#[ignore]`, so
the default `cargo test` stays green).

## Root cause

The element form derives `Q` from the destination **arrangement** (`arr_d`)
and `imm5` from the source **element size** (`elem_size`) **independently**,
so a mismatched pair is never checked:

```rust
let (q, _) = neon_arr_to_q_size(&arr_d)?;        // Q  from arrangement
...
let imm5 = match elem_size.as_str() { ... };     // imm5 (size) from lane
```

## Minimal input / expected / actual

```
dup v0.4s, v0.h[0]
  expected: Err
  actual:   Ok(Word(0x4E020400))   // Q=1 from .4s, imm5=0b00010 (halfword) from .h → invalid mix
```

## Impact

The ARM ARM requires the source element size to match the arrangement's
element size (`.4S ↔ .s[i]`). Encoding a mismatched pair produces an
architecturally UNDEFINED / differently-disassembled word. `llvm-mc` rejects
these with `"invalid operand for instruction"`. The encoder silently emits
such words, so a malformed source never surfaces.

## Suggested fix

After decoding both, assert the arrangement's element size equals the lane's
element size before encoding:

```
8b | 16b <-> b
4h | 8h  <-> h
2s | 4s  <-> s
1d | 2d  <-> d
```

Return `Err` otherwise.
