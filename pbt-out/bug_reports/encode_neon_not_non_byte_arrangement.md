# Bug Report: `encode_neon_not` silently accepts non-byte arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_not`
**Severity:** High

## Summary

`NOT` (alias of `MVN`, vector) is architecturally defined **only for byte
lanes** — `.8b` with Q=0 and `.16b` with Q=1. It belongs to the "Advanced SIMD
two-register miscellaneous" group, where `size` is a **fixed** field (`00`) of
the encoding, independent of the arrangement. The ARMv8-A ARM marks every other
element size UNALLOCATED for `MVN`/`NOT`.

The encoder, however, accepts **any** arrangement string and silently maps every
`arr != "16b"` to `Q=0`, emitting a valid-looking byte-`NOT` word. A mnemonic
like `not v0.4h, v1.4h` is therefore assembled to the *exact same* machine word
as `not v0.8b, v1.8b` — silently corrupting the program with no diagnostic. This
diverges from the reference assembler (LLVM), which rejects the non-byte forms
with `error: invalid operand for instruction`.

## Root Cause

`src/backend/arm/assembler/encoder/neon.rs:608`

```rust
pub(crate) fn encode_neon_not(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 2 {
        return Err("not requires 2 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };   // <-- any other arr -> Q=0

    // NOT Vd.T, Vn.T (alias of MVN): 0 Q 1 01110 00 10000 00101 10 Rn Rd
    let word = ((q << 30) | (1 << 29) | (0b01110 << 24))
        | (0b10000 << 17) | (0b00101 << 12) | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

There is no validation that `arr_d` is one of the two allocated arrangements
(`"8b"` / `"16b"`). `size` is hard-coded to `00`, and the only arrangement that
affects the encoding is `"16b"` (→ Q=1); every other string (including `.4h`,
`.8h`, `.2s`, `.4s`, `.1d`, `.2d`, and arbitrary garbage) collides with the
`.8b` encoding.

## Reproduction

LLVM's AArch64 assembler rejects every non-byte arrangement:

```
$ for arr in 4h 8h 2s 4s 1d 2d; do
    printf '.text\nnot v0.%s, v1.%s\n' $arr $arr > t.s
    clang --target=aarch64 -c t.s -o t.o
  done
4h   -> error: invalid operand for instruction
8h   -> error: invalid operand for instruction
2s   -> error: invalid operand for instruction
4s   -> error: invalid operand for instruction
1d   -> error: invalid operand for instruction
2d   -> error: invalid operand for instruction
```

This crate instead returns `Ok`:

```
not v0.4h, v1.4h  ->  Ok(EncodeResult::Word(0x2E205820))
                       (byte-for-byte identical to `not v0.8b, v1.8b`)
```

Run the `#[ignore]`d regression test to reproduce:

```
cargo test --lib neon_not_pbt::not_rejects_non_byte -- --ignored
```

Observed failure:

```
thread '...neon_not_pbt::not_rejects_non_byte' panicked at neon_not_pbt.rs:236:9:
NOT does not support .4h (only .8b/.16b are allocated);
expected Err but got Ok(0x2E205820)
```

## Impact

High — silent mis-assembly. A user writing `not`/`mvn` with a non-byte
arrangement gets a wrong instruction (a byte-`NOT`) with no error, silently
diverging from the reference assembler and the programmer's intent. A `.2d` /
`.4s` request silently degrades to `.8b`, operating on completely different
lanes. This matches the same class of bug previously reported for
`encode_neon_bsl` (see `encode_neon_bsl_non_byte_arrangement.md`) and
`encode_neon_pmul`.

## Suggested Fix

Reject any arrangement other than `"8b"` or `"16b"` before building the word:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;

let q: u32 = match arr_d.as_str() {
    "8b" => 0,
    "16b" => 1,
    other => return Err(format!(
        "not: arrangement '.{}' is not allocated (NOT is defined only for .8b/.16b)", other)),
};
```

Once fixed, the `#[ignore]`d witness flips to passing.

## Regression Property

Failing property: `not_rejects_non_byte` (module
`backend::arm::assembler::encoder::neon_not_pbt`, ignored).

```rust
#[test]
#[ignore]
fn not_rejects_non_byte() {
    for arr in &["4h", "8h", "2s", "4s", "1d", "2d"] {
        let ops = vec![va(0, arr), va(1, arr)];
        let res = encode_neon_not(&ops);
        assert!(
            res.is_err(),
            "NOT does not support .{arr} (only .8b/.16b are allocated); \
             expected Err but got Ok(0x{:08X})",
            res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0),
        );
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/240
