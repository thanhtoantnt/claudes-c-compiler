# Bug Report: `encode_neon_bitwise_insert` silently accepts non-byte arrangements

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_bitwise_insert`
**Severity:** High

## Summary

`BIT` (Bitwise Insert if True) and `BIF` (Bitwise Insert if False) are
architecturally defined **only for byte lanes** — `.8b` with Q=0 and `.16b` with
Q=1. They belong to the "Advanced SIMD three same" logical group
(AND/BIC/ORR/ORN/EOR/BSL/**BIT/BIF**), which is byte-only; the `size` field
selects the variant (`size=10` → `BIT`, `size=11` → `BIF`) and is otherwise
independent of the arrangement. The ARMv8-A ARM marks every other element size
UNALLOCATED for `BIT`/`BIF`.

The encoder, however, accepts **any** arrangement string and silently maps every
`arr != "16b"` to `Q=0`, emitting a valid-looking byte-`BIT`/`BIF` word. A
mnemonic like `bit v0.4h, v1.4h, v2.4h` is therefore assembled to the *exact
same* machine word as `bit v0.8b, v1.8b, v2.8b` — silently corrupting the
program with no diagnostic. This diverges from the reference assembler (LLVM),
which rejects the non-byte forms with `error: invalid operand for instruction`.
This is the same root cause as the previously-reported `encode_neon_bsl` finding
(`pbt-out/bug_reports/encode_neon_bsl_non_byte_arrangement.md`).

## Root Cause

```rust
pub(crate) fn encode_neon_bitwise_insert(operands: &[Operand], size: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bit/bif requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let q: u32 = if arr_d == "16b" { 1 } else { 0 };   // <-- any other arr -> Q=0
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

There is no validation that `arr_d` is one of the two allocated arrangements
(`"8b"` / `"16b"`). The only arrangement that affects the encoding is `"16b"`
(→ Q=1); every other string (including `.4h`, `.8h`, `.2s`, `.4s`, `.1d`,
`.2d`, and arbitrary garbage) collides with the `.8b` encoding. `size` is
passed through verbatim from the dispatcher
(`mod.rs:776-777`: `"bit" => …(operands, 0b10)`, `"bif" => …(operands, 0b11)`)
and is not range-checked either.

## Reproduction

LLVM's AArch64 assembler rejects every non-byte arrangement:

```
$ for arr in 4h 8h 2s 4s 1d 2d; do
    printf '.text\nbit v0.%s, v1.%s, v2.%s\n' $arr $arr $arr > t.s
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
bit v0.4h, v1.4h, v2.4h  ->  Ok(EncodeResult::Word(0x2EA21C20))
                              (byte-for-byte identical to `bit v0.8b, v1.8b, v2.8b`)
```

Run the `#[ignore]`d regression test to reproduce:

```
cargo test --lib -- --ignored bit_bif_reject_non_byte
```

Observed failure:

```
thread '...neon_bitwise_insert_pbt::bit_bif_reject_non_byte' panicked at neon_bitwise_insert_pbt.rs:281:13:
size=0b10 .4h: BIT/BIF only supports .8b/.16b; expected Err but got Ok(0x2EA21C20)
```

## Impact

High — silent mis-assembly. A user writing `bit`/`bif` with a non-byte
arrangement gets a wrong instruction (a byte-`BIT`/`BIF`) with no error,
silently diverging from the reference assembler and the programmer's intent.
`BIT`/`BIF` are commonly used in conditional-bit-update / mask-selection
idioms (often hand-written in assembly); a `.2d`/`.4s` request silently degrades
to `.8b`, operating on completely different lanes. This matches the same class
of bug previously reported for `encode_neon_bsl`, `encode_neon_pmul`
(`encode_neon_pmul_non_byte_arrangement.md`), and `encode_neon_mla`. The sibling
logical encoders in this file (`encode_neon_bic`, `encode_neon_logical`) share
the latent `if arr == "16b"` collapse, though they are out of scope for this
report.

## Suggested Fix

Reject any arrangement other than `"8b"` or `"16b"` (and optionally assert
`size ∈ {0b10, 0b11}`) before building the word:

```rust
pub(crate) fn encode_neon_bitwise_insert(operands: &[Operand], size: u32) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("bit/bif requires 3 operands".to_string());
    }
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;

    let q: u32 = match arr_d.as_str() {
        "8b" => 0,
        "16b" => 1,
        other => return Err(format!(
            "bit/bif: arrangement '.{}' is not allocated (BIT/BIF are defined only for .8b/.16b)", other)),
    };
    if size != 0b10 && size != 0b11 {
        return Err(format!("bit/bif: invalid size field {:#04b}", size));
    }

    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b000111 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Regression Property

Failing property: `bit_bif_reject_non_byte` (module
`backend::arm::assembler::encoder::neon_bitwise_insert_pbt`, ignored).

```rust
#[test]
#[ignore]
fn bit_bif_reject_non_byte() {
    for size in [0b10u32, 0b11u32] {
        for arr in &["4h", "8h", "2s", "4s", "1d", "2d"] {
            let ops = vec![va(0, arr), va(1, arr), va(2, arr)];
            let res = encode_neon_bitwise_insert(&ops, size);
            let got = res.as_ref().ok().map(|e| match e {
                EncodeResult::Word(w) => *w,
                _ => 0,
            }).unwrap_or(0);
            assert!(
                res.is_err(),
                "size={size:#04b} .{arr}: BIT/BIF only supports .8b/.16b; \
                 expected Err but got Ok(0x{got:08X})",
            );
        }
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/293
