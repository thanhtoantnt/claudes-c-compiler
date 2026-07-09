# Bug Report: `encode_neon_addv` silently accepts architecturally invalid arrangements (`.1d`/`.2d`/`.2s`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_addv`
**Severity:** Medium

## Summary

`ADDV` (vector) is defined by the ARMv8-A ARM **only** for the arrangements
`8B, 16B, 4H, 8H, 4S` (sizes `0b00/0b01/0b10`). The `.1d`/`.2d` arrangements
(`size = 0b11`) and `.2s` are **not** valid for `ADDV` and the corresponding
encodings are UNALLOCATED. `encode_neon_addv` accepts them and emits a word
instead of returning `Err`, producing silently-wrong / unallocated machine
code.

## Root Cause

`encode_neon_addv` derives `(q, size)` unconditionally from the source
arrangement via `neon_arr_to_q_size` and performs no validity check on the
arrangement:

```rust
pub(crate) fn encode_neon_addv(operands: &[Operand]) -> Result<EncodeResult, String> {
    ...
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, arr_n) = get_neon_reg(operands, 1)?;

    let (q, size) = neon_arr_to_q_size(&arr_n)?;   // succeeds for 1d/2d/2s
    // no check that arr_n is one of 8B/16B/4H/8H/4S
    let word = (q << 30) | ... ;
    Ok(EncodeResult::Word(word))
}
```

`neon_arr_to_q_size` happily maps `1d → (0, 0b11)`, `2d → (1, 0b11)`, and
`2s → (0, 0b10)`, so the invalid arrangements flow through and a word is
emitted. There is no spec-conformance guard.

## Reproduction

```text
$ cargo test --lib addv_rejects_unallocated_arrangements -- --ignored
test ...::addv_rejects_unallocated_arrangements ... FAILED
  ADDV does not support .1d; expected Err but got Ok(0x0EF0DC20)
```

The property asserts that `.1d`, `.2d`, and `.2s` must each return `Err`;
all three currently return `Ok`. Witness: `addv v0.1d, v1.1d` →
`Ok(0x0EF0DC20)` (a word whose `size` field is `0b11`, the UNALLOCATED case
for ADDV).

## Impact

Medium. Invalid arrangements are silently encoded instead of rejected. A user
writing `.1d`/`.2d`/`.2s` gets unallocated machine code with no diagnostic.
This is a negative-contract gap, distinct from (and independent of) the
opcode-field mis-assembly in
`encode_neon_addv_wrong_opcode_field.md`; even after the opcode fix, these
arrangements must still be rejected. It matches the pre-existing pattern
documented for the sibling `encode_neon_mla`
(`encode_neon_mla_unallocated_doubleword.md`).

## Suggested Fix

After extracting the arrangement, validate it against the allowed set:

```rust
const ADDV_ARRANGEMENTS: &[&str] = &["8b", "16b", "4h", "8h", "4s"];
if !ADDV_ARRANGEMENTS.contains(&arr_n.as_str()) {
    return Err(format!("addv: unsupported arrangement: {} \
        (ADDV is defined only for 8B/16B/4H/8H/4S)", arr_n));
}
```

After the fix, the `#[ignore]`d test `addv_rejects_unallocated_arrangements`
passes and can be un-ignored.

## Regression Property

Failing property: `addv_rejects_unallocated_arrangements`

```rust
#[test]
#[ignore]
fn addv_rejects_unallocated_arrangements() {
    for arr in &["1d", "2d", "2s"] {
        let ops = vec![va(0, arr), va(1, arr)];
        let res = encode_neon_addv(&ops);
        assert!(
            res.is_err(),
            "ADDV does not support .{arr}; expected Err but got Ok(0x{:08X})",
            res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0),
        );
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/190
