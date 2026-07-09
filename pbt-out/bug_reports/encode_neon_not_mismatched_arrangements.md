# Bug Report: `encode_neon_not` silently accepts mismatched `Vd`/`Vn` arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_not`
**Severity:** Medium

## Summary

For a vector `NOT`/`MVN`, the destination and source operands must carry the
**same** arrangement (both `.8b` or both `.16b`). The ARMv8-A ARM ties the two
operands' `<T>` together, and the reference assembler (LLVM) rejects any
mismatch with `error: invalid operand for instruction`.

The encoder, however, discards the source operand's arrangement entirely
(`let (rn, _) = ...`) and derives the `Q` bit solely from the *destination*
arrangement. A mnemonic like `not v0.8b, v1.16b` is therefore assembled with no
error, producing the `.8b` encoding while silently ignoring the source's `.16b`
arrangement — a wrong-operand-width instruction with no diagnostic.

## Root Cause

`src/backend/arm/assembler/encoder/neon.rs:608`

```rust
pub(crate) fn encode_neon_not(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("not requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;   // <-- source arrangement discarded

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };

    // NOT Vd.T, Vn.T (alias of MVN): 0 Q 1 01110 00 10000 00101 10 Rn Rd
    let word = ((q << 30) | (1 << 29) | (0b01110 << 24))
        | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The source arrangement is bound with `_` and never compared to `arr_d`, so any
mismatch between the two byte arrangements is silently coerced to the
destination's form.

## Reproduction

LLVM's AArch64 assembler rejects the mismatched forms:

```
$ printf '.text\nnot v0.8b, v1.16b\n' > t.s && clang --target=aarch64 -c t.s -o t.o
<stdin>:1:12: error: invalid operand for instruction
$ printf '.text\nmvn v0.16b, v1.8b\n' > t.s && clang --target=aarch64 -c t.s -o t.o
<stdin>:1:13: error: invalid operand for instruction
```

This crate instead returns `Ok`:

```
not v0.8b, v1.16b  ->  Ok(EncodeResult::Word(0x2E205820))
                        (the .8b encoding, source .16b silently ignored)
```

Run the `#[ignore]`d regression test to reproduce:

```
cargo test --lib neon_not_pbt::not_rejects_mismatched_arrangements -- --ignored
```

Observed failure:

```
thread '...neon_not_pbt::not_rejects_mismatched_arrangements' panicked at neon_not_pbt.rs:266:9:
NOT requires Vd and Vn to share arrangement; not v0.8b, v1.16b:
expected Err but got Ok(0x2E205820)
```

## Impact

Medium — silent mis-assembly of an operand-width mismatch. A mismatched
`NOT Vd.8b, Vn.16b` is emitted as if both were `.8b`, with no diagnostic,
diverging from the reference assembler and the programmer's intent. This is a
distinct defect from the non-byte acceptance bug (see
`encode_neon_not_non_byte_arrangement.md`); both stem from the same missing
arrangement validation, but this one fires on otherwise-allocated byte
arrangements that merely disagree between source and destination.

## Suggested Fix

Bind the source arrangement and require it to match the destination:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;

if arr_d != arr_n {
    return Err(format!(
        "not: arrangement mismatch .{arr_d} vs .{arr_n} (Vd and Vn must match)"));
}

let q: u32 = if arr_d == "16b" { 1 } else { 0 };
```

Once fixed, the `#[ignore]`d witness flips to passing.

## Regression Property

Failing property: `not_rejects_mismatched_arrangements` (module
`backend::arm::assembler::encoder::neon_not_pbt`, ignored).

```rust
#[test]
#[ignore]
fn not_rejects_mismatched_arrangements() {
    for (ad, an) in &[("8b", "16b"), ("16b", "8b")] {
        let ops = vec![va(0, ad), va(1, an)];
        let res = encode_neon_not(&ops);
        assert!(
            res.is_err(),
            "NOT requires Vd and Vn to share arrangement; \
             not v0.{ad}, v1.{an}: expected Err but got Ok(0x{:08X})",
            res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0),
        );
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/239
