# Bug Report: `encode_neon_elem` silently wraps out-of-range lane index and halfword Rm

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_elem`
**Severity:** High

## Summary

`encode_neon_elem` encodes the AArch64 NEON *Advanced SIMD vector by element*
(non-long) multiply family — `MUL`/`MLA`/`MLS`/`SQDMULH`/`SQRDMULH` (by element).
The instruction's lane `index` and (for the halfword form) the source register
`Rm` occupy a fixed number of bits, so the ARMv8-A ARM bounds both. This encoder
never validates either bound: it **masks** out-of-range values instead of
rejecting them. As a result:

* A halfword lane index > 7 silently aliases index 0–7; a word lane index > 3
  silently aliases index 0–3.
* For the halfword form, a source register `V16`–`V31` silently aliases
  `V0`–`V15`.

Both are silent mis-encodings — the function returns `Ok(Word(...))` with a
wrong but well-formed instruction, so the assembler produces code that assembles
cleanly yet selects the wrong vector lane / wrong source register.

The crate's own sibling `encode_neon_elem_long` enforces the lane-index bounds
(`neon.rs:266` and `neon.rs:273`: `if index > 7 { return Err(...) }` / `if
index > 3 { return Err(...) }`), establishing that the intended contract is to
reject — `encode_neon_elem` simply omits the same checks.

## Root Cause

```rust
// src/backend/arm/assembler/encoder/neon.rs:1600
let (h, l, m_bit) = match size {
    0b01 => ((index >> 2) & 1, (index >> 1) & 1, index & 1),   // halfword: no range check on `index`
    0b10 => ((index >> 1) & 1, index & 1, (rm >> 4) & 1),      // word:     no range check on `index`
    _ => return Err("unsupported element size for by-element".to_string()),
};
let rm_enc = if size == 0b01 { rm & 0xF } else { rm & 0x1F };  // halfword: rm & 0xF silently drops bit 4
```

The bit extractions `index >> n & 1` discard high bits without consulting
`index`; `rm & 0xF` discards `Rm[4]` without consulting `rm`. Contrast with the
long form, which returns `Err` for the same conditions.

## Reproduction

Both bug witnesses are `#[ignore]`d in
`src/backend/arm/assembler/encoder/neon_elem_pbt.rs` so the default `cargo test`
stays green. Run them with `cargo test --lib neon_elem_pbt -- --ignored`.
`proptest` shrinks each to the minimal offending input.

```
// halfword index 8 silently aliases index 0
encode_neon_elem(&[va(0,"4h"), va(1,"4h"), lane(2,"h",8)], 0, 0b1000)
  == encode_neon_elem(&[va(0,"4h"), va(1,"4h"), lane(2,"h",0)], 0, 0b1000)
  == Ok(Word(0x0F428020))   // index 8 → H:L:M = 0:0:0

// word index 4 silently aliases index 0
encode_neon_elem(&[va(0,"4s"), va(1,"4s"), lane(2,"s",4)], 0, 0b1000)
  == encode_neon_elem(&[va(0,"4s"), va(1,"4s"), lane(2,"s",0)], 0, 0b1000)
  == Ok(Word(0x0F828020))   // index 4 → H:L = 0:0

// halfword Rm v16 silently aliases v0
encode_neon_elem(&[va(0,"4h"), va(1,"4h"), lane(16,"h",0)], 0, 0b1000)
  == encode_neon_elem(&[va(0,"4h"), va(1,"4h"), lane(0,"h",0)], 0, 0b1000)
  == Ok(Word(0x0F400020))   // rm 16 & 0xF == 0
```

**Expected:** each `Ok(Word(...))` should instead be `Err("... out of range ...")`
(matching `encode_neon_elem_long`). **Actual:** all return `Ok` with a
wrong-but-well-formed word.

`prop_out_of_range_index_must_error` minimal failing input: `idx_h = 8, idx_s = 4`.
`prop_halfword_rm_above_v15_must_be_rejected` minimal failing input: `rm = 16`.

## Impact

Silent mis-compilation of any of the five by-element instructions when the source
references a lane/register outside the encodable range. The assembler neither
errors nor warns, so the bug surfaces only as incorrect runtime behaviour (wrong
element multiplied/accumulated, wrong source lane), which is extremely hard to
diagnose. There is no crash — the encoding is architecturally *valid*, just for
the wrong operand — so no CI or sanitizer will flag it. Codegen bugs that choose
a wrong `SQDMULH`/`MLA`/`MUL` lane would directly corrupt vector arithmetic.

## Suggested Fix

Mirror the long form's validation before extracting the index/Rm bits:

```rust
let (h, l, m_bit) = match size {
    0b01 => {
        if index > 7 { return Err(format!("element index {} out of range for .h", index)); }
        if rm > 15  { return Err(format!("element register v{} out of range for .h (V0-V15)", rm)); }
        ((index >> 2) & 1, (index >> 1) & 1, index & 1)
    }
    0b10 => {
        if index > 3 { return Err(format!("element index {} out of range for .s", index)); }
        ((index >> 1) & 1, index & 1, (rm >> 4) & 1)
    }
    _ => return Err("unsupported element size for by-element".to_string()),
};
let rm_enc = if size == 0b01 { rm & 0xF } else { rm & 0x1F };
```

## Regression Property

Failing property: `prop_out_of_range_index_must_error`,
`prop_halfword_rm_above_v15_must_be_rejected`

```rust
#[test]
#[ignore = "documented bug: out-of-range by-element index silently wraps (no Err)"]
fn prop_out_of_range_index_must_error(idx_h in 8u32..=0xFFFF, idx_s in 4u32..=0xFFFF) {
    let h_ops = vec![va(0, "4h"), va(1, "4h"), lane(2, "h", idx_h)];
    prop_assert!(encode_neon_elem(&h_ops, 0, 0b1000).is_err());
    let s_ops = vec![va(0, "4s"), va(1, "4s"), lane(2, "s", idx_s)];
    prop_assert!(encode_neon_elem(&s_ops, 0, 0b1000).is_err());
}

#[test]
#[ignore = "documented bug: halfword by-element Rm V16-V31 aliases V0-V15"]
fn prop_halfword_rm_above_v15_must_be_rejected(rm in 16u32..=31) {
    let ops = vec![va(0, "4h"), va(1, "4h"), lane(rm, "h", 0)];
    prop_assert!(encode_neon_elem(&ops, 0, 0b1000).is_err());
}
```

Once fixed, drop the `#[ignore]` attributes so these become permanent regression
guards. (A softer, separate finding — the lane operand's `elem_size` is never
validated against the arrangement — is recorded by the `elem_size_mismatch_accepted`
witness in the same file.)


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/256
