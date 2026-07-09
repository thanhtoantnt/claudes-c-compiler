# Bug Report: `encode_neon_pmul` silently accepts non-byte arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_pmul`
**Severity:** High

## Summary

`PMUL` (polynomial multiply, vector) is architecturally defined **only for byte
lanes** — `.8b` with Q=0 and `.16b` with Q=1 (size=00). The ARMv8-A ARM
"Advanced SIMD three same" group marks every other element size UNALLOCATED for
`PMUL`.

The encoder, however, accepts **any** arrangement string and silently maps every
`arr != "16b"` to `Q=0`, emitting a valid-looking byte-`PMUL` word. A mnemonic
like `pmul v0.4h, v1.4h, v2.4h` is therefore assembled to the *exact same*
machine word as `pmul v0.8b, v1.8b, v2.8b` — silently corrupting the program
with no diagnostic. This diverges from the reference assembler (LLVM), which
rejects the non-byte forms with `error: invalid operand for instruction`.

## Root Cause

```rust
pub(crate) fn encode_neon_pmul(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };   // <-- any other arr -> Q=0
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (1 << 21)
        | (rm << 16) | (0b100111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

There is no validation that `arr_d` is one of the two allocated arrangements
(`"8b"` / `"16b"`). `size` is implicitly `00`, and the only arrangement that
affects the encoding is `"16b"` (→ Q=1); every other string (including `.4h`,
`.8h`, `.2s`, `.4s`, `.1d`, `.2d`, and arbitrary garbage) collides with the
`.8b` encoding.

## Reproduction

LLVM's AArch64 assembler rejects every non-byte arrangement:

```
$ for arr in 4h 8h 2s 4s 1d 2d; do
    printf '.text\npmul v0.%s, v1.%s, v2.%s\n' $arr $arr $arr > t.s
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
pmul v0.4h, v1.4h, v2.4h  ->  Ok(EncodeResult::Word(0x2E229C20))
                              (byte-for-byte identical to `pmul v0.8b, v1.8b, v2.8b`)
```

Run the `#[ignore]`d regression test to reproduce:

```
cargo test --lib -- --ignored pmul_rejects_non_byte
```

## Impact

High — silent mis-assembly. A user writing `pmul` with a non-byte arrangement
gets a wrong instruction (a byte-`PMUL`) with no error, silently diverging from
the reference assembler and the programmer's intent. For cryptographic code
(`PMUL` is used in GF(2^m) multiplication, e.g. GCM), this corrupts the
computation undetectably. This matches the same class of bug previously
reported for `encode_neon_mla` (see
`pbt-out/bug_reports/encode_neon_mla_unallocated_doubleword.md`).

## Suggested Fix

Reject any arrangement other than `"8b"` or `"16b"` before building the word:

```rust
pub(crate) fn encode_neon_pmul(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let q: u32 = match arr_d.as_str() {
        "8b" => 0,
        "16b" => 1,
        other => return Err(format!(
            "pmul: arrangement '.{}' is not allocated (PMUL is defined only for .8b/.16b)", other)),
    };
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (1 << 21)
        | (rm << 16) | (0b100111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Regression Property

Failing property: `pmul_rejects_non_byte` (module
`backend::arm::assembler::encoder::neon_pmul_pbt`, ignored).

```rust
#[test]
#[ignore]
fn pmul_rejects_non_byte() {
    for arr in &["4h", "8h", "2s", "4s", "1d", "2d"] {
        let ops = vec![va(0, arr), va(1, arr), va(2, arr)];
        let res = encode_neon_pmul(&ops);
        assert!(
            res.is_err(),
            "PMUL does not support .{arr} (only .8b/.16b are allocated); \
             expected Err but got Ok(0x{:08X})",
            res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0),
        );
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/198
