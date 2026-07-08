# Bug — `encode_blr`: FP/SIMD register class wrongly accepted

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_blr`

```rust
pub(crate) fn encode_blr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let word = 0xd63f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

The shared helper `parse_reg_num` (in `encoder/mod.rs`) accepts the FP/SIMD register prefixes as if they were general-purpose:

```rust
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```

So `blr d0`, `blr s3`, `blr q7`, `blr v5`, `blr h15`, `blr b2`, etc. are parsed and emit `blr x{n}` — the FP/SIMD register number is **silently reused** as a general-purpose register number. `BLR`'s operand is a **general-purpose** `<Xn>` register (ARM ARM C5.6.18); FP/SIMD operands are the **wrong register class** and are unallocated encodings a conforming assembler must reject:

```
$ echo "blr d0" | llvm-mc -triple=aarch64 -show-encoding
error: invalid operand for instruction
```

## Minimal input

| Mnemonic | Encoded word | Expected |
|---|---|---|
| `blr d0` | `0xD63F0000` (== `blr x0`) | `Err` (wrong register class) |
| `blr v5` | `0xD63F0000 | (5<<5)` (== `blr x5`) | `Err` |
| `blr q31` | `0xD63F0000 | (31<<5)` | `Err` |

## Actual behavior (observed failure)

`encode_blr(&[Operand::Reg("d0".into())])` returns `Ok(EncodeResult::Word(0xD63F0000))`, bit-identical to `encode_blr(&[Operand::Reg("x0".into())])`. The FP/SIMD register is silently reinterpreted as a GP register of the same number; no diagnostic. Confirmed by a **failing** proptest run:

```
prop_rejects_fp_simd_registers
  panicked: blr d0 must be rejected (...), got Ok(Word(3594452992))
  minimal failing input: prefix_idx = 0, n = 0   (3594452992 == 0xD63F0000)
```

## Impact

Silent mis-assembly producing semantically wrong code with no assembler error. Any toolchain consumer that validates by "the assembler accepted it" is misled into thinking a GP register was used. The identical defect affects the sibling `encode_br` and every other encoder that consumes `parse_reg_num` without re-checking the register class.

## Property that locks it (FAILING — bug confirmed)

`prop_encode_blr_tests::prop_rejects_fp_simd_registers` (in `compare_branch.rs`) is a **negative-contract** property asserting `encode_blr({d,s,q,v,h,b}N).is_err()`. It **FAILS** against the current implementation (`blr d0 → Ok(0xD63F0000)`). Once validation is added the property will pass; no assertion changes are needed.

## Fix

```rust
let name = match operands.get(0) {
    Some(Operand::Reg(r)) => r.to_lowercase(),
    _ => return Err("blr: expected register".into()),
};
if is_fp_reg(&name) {
    return Err(format!("blr: operand must be a general-purpose register, got {}", name));
}
```

(`is_fp_reg` already exists in `encoder/mod.rs` and matches prefixes `d | s | q | v | h | b`.)

## Regression property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_blr(&[Operand::Reg("d0".into())]).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/13
