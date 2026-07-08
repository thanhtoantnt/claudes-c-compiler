# Bug — `encode_br` accepts FP/SIMD registers (wrong register class)

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_br`
**Severity:** Correctness / assembler-conformance (silent mis-assembly)
**Pinned by:** `prop_encode_br_tests::prop_fp_simd_registers_wrongly_accepted`

## Summary

`encode_br` operates only on a general-purpose `X` register, but the shared
`parse_reg_num` helper accepts the FP/SIMD register prefixes
(`d`/`s`/`q`/`v`/`h`/`b`). As a result `br d0`, `br v5`, `br s3`, `br q7`,
`br h1`, `br b2` are all **accepted** and encode **identically** to `br x{N}`
— a completely different instruction — instead of being rejected as
unallocated encodings.

```rust
pub(crate) fn encode_br(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let word = 0xd61f0000 | (rn << 5);     // rn from a d/v/s/q/h/b name
    Ok(EncodeResult::Word(word))
}
```

`parse_reg_num` (`encoder/mod.rs`) accepts these prefixes:
```rust
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```

## Minimal input

```
br d0
br v5
br s3
```

## Expected vs actual

| input | **Expected** (GAS / `llvm-mc`) | **Actual** |
|---|---|---|
| `br d0` | `Err` — `operand 1 must be an integer register` | `Ok(Word(0xd61f0000))` == `br x0` |
| `br v5` | `Err` — FP/SIMD is the wrong register class | `Ok(Word(0xd61f0028))` == `br x5` |
| `br s3` | `Err` | `Ok(Word(0xd61f0018))` == `br x3` |

A FP/SIMD register and the GP register with the same number are completely
unrelated; encoding `br d0` as `br x0` mis-assembles the program with no
diagnostic.

## Impact

Silent mis-assembly: a branch through a floating-point/SIMD register name is
emitted as a branch through an unrelated GP register, producing wrong control
flow with no error.

## Root cause

`parse_reg_num` is register-class-agnostic: it accepts every register prefix.
`BR` is a GP-only instruction and must reject FP/SIMD operands.

## Suggested fix

`encode_br` (and sibling `encode_blr`, `encode_ret`) should reject FP/SIMD
register names. E.g. guard with `is_fp_reg` (already defined in
`encoder/mod.rs`), or restrict accepted names to the GP family (`x*`, `sp`,
`xzr`, `lr`):

```rust
let name = match operands.get(0) {
    Some(Operand::Reg(n)) => n,
    _ => return Err("br requires a register".into()),
};
if is_fp_reg(name) {
    return Err("br requires a general-purpose X register, not FP/SIMD".into());
}
let (rn, is_64) = get_reg(operands, 0)?;
if !is_64 { return Err("br requires a 64-bit X register".into()); }
let word = 0xd61f0000 | (rn << 5);
Ok(EncodeResult::Word(word))
```

When landed, `prop_fp_simd_registers_wrongly_accepted`'s `is_ok()` assertion
should flip to `is_err()`.

## Reproduce
```bash
cargo test --lib prop_fp_simd_registers_wrongly_accepted
```
