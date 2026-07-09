# Bug Report: `encode_ldar_stlr` silently accepts FP/SIMD registers

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldar_stlr`
**Severity:** Medium
**Aspect:** Register-class validation

## Summary

`encode_ldar_stlr` (LDAR/STLR/LDARB/STLRB/LDARH/STLRH — Load-Acquire /
Store-Release Register, ARM ARM §C6.2.101 / §C6.2.275) only operates on
**general-purpose** registers. The data register `<Rt>` MUST be a W or X
register; placing an FP/SIMD register (B/H/S/D/V/Q) in the Rt position is
architecturally **UNALLOCATED** and must be rejected with `Err`.

The encoder performs no register-class check. It calls `get_reg(operands, 0)`,
whose underlying `parse_reg_num` happily recognises every FP/SIMD prefix
(`d`/`s`/`q`/`v`/`h`/`b`, see `mod.rs:311`), and never consults `is_fp_reg`.
As a result an FP/SIMD register is silently accepted and — because
`is_64bit_reg("<fp>")` is `false` — encodes identically to the corresponding
**W** register, producing an instruction the architecture treats as a valid
GP load/store on a *different* register bank with no diagnostic.

## Root Cause

```rust
pub(crate) fn encode_ldar_stlr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    let (rt, is_64) = get_reg(operands, 0)?;          // <-- accepts FP/SIMD
    let rn = match operands.get(1) {
        Some(Operand::Mem { base, .. }) => parse_reg_num(base).ok_or("invalid base")?,
        _ => return Err("ldar/stlr needs memory operand".to_string()),
    };
    let size = forced_size.unwrap_or(if is_64 { 0b11 } else { 0b10 }); // is_64 == false for FP
    ...
}
```

There is no `is_fp_reg(&operands[0])` guard before encoding. This is the same
defect class already reported for the sibling branch instructions
(`encode_br_accepts_fp_simd_registers.md`,
`encode_blr-fpsimd-register-class.md`, `encode_ret-fpsimd-register-class.md`).

## Reproduction

**Minimal input:** `stlr d0, [x0]` (equivalently `ldar d0, [x0]`,
or any of `s0`/`q0`/`v0`/`h0`/`b0`).

**Expected:** `Err` — an FP/SIMD register in the Rt position is UNALLOCATED.

**Actual:** `Ok(Word(2292186112))` — silently accepted. The encoded word is
bit-for-bit identical to `stlr w0, [x0]` because `is_64bit_reg("d0") == false`,
so the FP register aliases the 32-bit GP form.

Shrunk proptest counterexample:
```
is_load = false, prefix = 'd', rt_num = 0, base_num = 0   →  stlr d0, [x0]
```

The aliasing is total: `ldar d0/s0/q0/v0/h0/b0, [x1]` all produce the *same*
word (`2296380448`), indistinguishable from `ldar w0, [x1]`.

## Impact

Silent mis-encoding with no diagnostic. An assembler user (or a programmatic
caller of the encoder) who writes `ldar d0, [x1]` expecting either an error or
a genuine FP acquire-load instead gets the encoding of `ldar w0, [x1]` —
touching the wrong register bank. This is the worst class of assembler bug:
valid-looking output, no error, wrong semantics. Severity is bounded only
because today's mnemonic dispatch (`mod.rs:540-545`) never feeds FP operands
to this function from parsed assembly text, so it is latent for the text
assembler but live for any direct/internal caller.

## Suggested Fix

Validate the data register class before encoding, mirroring the GP-only
contract:

```rust
let rt_name = match &operands[0] { Operand::Reg(r) => r.as_str(), _ => "" };
if is_fp_reg(rt_name) {
    return Err(format!("ldar/stlr requires a general-purpose register, got {}", rt_name));
}
let (rt, is_64) = get_reg(operands, 0)?;
```

## Regression Property

Failing witness (marked `#[ignore]` so the default `cargo test` stays green;
run it explicitly to reproduce):

```
cargo test --lib load_store_ldar_ldxr_class_pbt::prop_ldar_stlr_fp_simd_register_rejected -- --ignored
```

```rust
#[test]
#[ignore = "documented bug: ldar/stlr silently accepts FP/SIMD registers (register-class)"]
fn prop_ldar_stlr_fp_simd_register_rejected(...) {
    let res = encode_ldar_stlr(&[Operand::Reg("d0".into()),
                                 Operand::Mem { base: "x0".into(), offset: 0 }],
                                false, None);
    prop_assert!(res.is_err());
}
```

The companion mechanism property
`prop_ldar_stlr_fp_simd_aliases_w` documents the aliasing smoking gun and is
also `#[ignore]`. Once the guard is added, both flip to passing and can be
un-ignored.

**GitHub Issue:** (to be filed)
