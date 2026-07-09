# Bug Report: `encode_neon_bsl` silently accepts non-byte arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_bsl`
**Severity:** High

## Summary

`BSL` (Bitwise Select, vector) is architecturally defined **only for byte
lanes** — `.8b` with Q=0 and `.16b` with Q=1. It belongs to the "Advanced SIMD
three same" logical group (AND/BIC/ORR/ORN/EOR/**BSL**/BIT/BIF), which is
byte-only; `size` is a **fixed** field (`01`) of the encoding, independent of
the arrangement. The ARMv8-A ARM marks every other element size UNALLOCATED for
`BSL`.

The encoder, however, accepts **any** arrangement string and silently maps every
`arr != "16b"` to `Q=0`, emitting a valid-looking byte-`BSL` word. A mnemonic
like `bsl v0.4h, v1.4h, v2.4h` is therefore assembled to the *exact same*
machine word as `bsl v0.8b, v1.8b, v2.8b` — silently corrupting the program
with no diagnostic. This diverges from the reference assembler (LLVM), which
rejects the non-byte forms with `error: invalid operand for instruction`.

## Root Cause

```rust
pub(crate) fn encode_neon_bsl(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bsl requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q: u32 = if arr_d == "16b" { 1 } else { 0 };   // <-- any other arr -> Q=0

    // BSL Vd.T, Vn.T, Vm.T: 0 Q 1 01110 01 1 Rm 000111 Rn Rd
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

There is no validation that `arr_d` is one of the two allocated arrangements
(`"8b"` / `"16b"`). `size` is hard-coded to `01`, and the only arrangement that
affects the encoding is `"16b"` (→ Q=1); every other string (including `.4h`,
`.8h`, `.2s`, `.4s`, `.1d`, `.2d`, and arbitrary garbage) collides with the
`.8b` encoding.

## Reproduction

LLVM's AArch64 assembler rejects every non-byte arrangement:

```
$ for arr in 4h 8h 2s 4s 1d 2d; do
    printf '.text\nbsl v0.%s, v1.%s, v2.%s\n' $arr $arr $arr > t.s
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
bsl v0.4h, v1.4h, v2.4h  ->  Ok(EncodeResult::Word(0x2E621C20))
                              (byte-for-byte identical to `bsl v0.8b, v1.8b, v2.8b`)
```

Run the `#[ignore]`d regression test to reproduce:

```
cargo test --lib -- --ignored bsl_rejects_non_byte
```

Observed failure:

```
thread '...neon_bsl_pbt::bsl_rejects_non_byte' panicked at neon_bsl_pbt.rs:241:9:
BSL does not support .4h (only .8b/.16b are allocated);
expected Err but got Ok(0x2E621C20)
```

## Impact

High — silent mis-assembly. A user writing `bsl` with a non-byte arrangement
gets a wrong instruction (a byte-`BSL`) with no error, silently diverging from
the reference assembler and the programmer's intent. `BSL` is commonly used in
bit-selection / conditional-move-by-mask idioms (often hand-written in
assembly); a `.2d`/`.4s` request silently degrades to `.8b`, operating on
completely different lanes. This matches the same class of bug previously
reported for `encode_neon_pmul` (see
`pbt-out/bug_reports/encode_neon_pmul_non_byte_arrangement.md`) and
`encode_neon_mla`. The sibling logical encoders in this file
(`encode_neon_bic`, `encode_neon_logical`) share the latent
`if arr == "16b"` collapse, though they are out of scope for this report.

## Suggested Fix

Reject any arrangement other than `"8b"` or `"16b"` before building the word:

```rust
pub(crate) fn encode_neon_bsl(operands: &[Operand]) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bsl requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q: u32 = match arr_d.as_str() {
        "8b" => 0,
        "16b" => 1,
        other => return Err(format!(
            "bsl: arrangement '.{}' is not allocated (BSL is defined only for .8b/.16b)", other)),
    };

    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (0b01 << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Regression Property

Failing property: `bsl_rejects_non_byte` (module
`backend::arm::assembler::encoder::neon_bsl_pbt`, ignored).

```rust
#[test]
#[ignore]
fn bsl_rejects_non_byte() {
    for arr in &["4h", "8h", "2s", "4s", "1d", "2d"] {
        let ops = vec![va(0, arr), va(1, arr), va(2, arr)];
        let res = encode_neon_bsl(&ops);
        assert!(
            res.is_err(),
            "BSL does not support .{arr} (only .8b/.16b are allocated); \
             expected Err but got Ok(0x{:08X})",
            res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0),
        );
    }
}
```
