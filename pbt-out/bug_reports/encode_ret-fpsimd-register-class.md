# Bug — `encode_ret`: FP/SIMD register class wrongly accepted

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_ret`

```rust
pub(crate) fn encode_ret(operands: &[Operand]) -> Result<EncodeResult, String> {
    let rn = if operands.is_empty() {
        30 // default to x30 (LR)
    } else {
        get_reg(operands, 0)?.0
    };
    let word = 0xd65f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

The shared helper `parse_reg_num` (in `encoder/mod.rs`) accepts the FP/SIMD
register prefixes as if they were general-purpose:

```rust
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```

So `ret d0`, `ret s3`, `ret q7`, `ret v5`, `ret h15`, `ret b2`, etc. are parsed and
emit `ret x{n}` — the FP/SIMD register number is **silently reused** as a
general-purpose register number. `RET`'s operand is a **general-purpose** `<Xn>`
register (ARM ARM C5.6.20); FP/SIMD operands are the **wrong register class** and
are unallocated encodings a conforming assembler must reject:

```
$ echo "ret d0"  | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
$ echo "ret v5"  | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
$ echo "ret q31" | clang --target=aarch64 -c -x assembler - -o /dev/null
-:1:5: error: invalid operand for instruction
```

## Minimal input

| Mnemonic | Encoded word | Reference (clang) | Expected here |
|---|---|---|---|
| `ret d0`  | `0xD65F0000` (== `ret x0`) | error: invalid operand | `Err` |
| `ret v5`  | `0xD65F0000 \| (5<<5)` (== `ret x5`) | error | `Err` |
| `ret q31` | `0xD65F0000 \| (31<<5)` (== `ret xzr`) | error | `Err` |

## Actual behavior (observed failure)

`encode_ret(&[Operand::Reg("d0".into())])` returns
`Ok(EncodeResult::Word(3596550144))` — `3596550144 == 0xD65F0000`, bit-identical to
`encode_ret(&[Operand::Reg("x0".into())])`. The FP/SIMD register is silently
reinterpreted as a GP register of the same number; no diagnostic. Confirmed by a
**failing** proptest:

```
prop_rejects_fp_simd_registers
  panicked: ret d0 must be rejected (FP/SIMD register; RET requires a GP register),
            got Ok(Word(3596550144))
  minimal failing input: prefix_idx = 0, n = 0   (3596550144 == 0xD65F0000)
```

## Impact

Silent mis-assembly producing semantically wrong code with no assembler error. Any
toolchain consumer that validates by "the assembler accepted it" is misled into
thinking a GP register was used. The identical defect affects the sibling
`encode_br`, `encode_blr`, and every other encoder that consumes `parse_reg_num`
without re-checking the register class.

## Property that locks it (FAILING — bug confirmed)

`prop_encode_ret_tests::prop_rejects_fp_simd_registers` (in `compare_branch.rs`)
is a **negative-contract** property asserting
`encode_ret({d,s,q,v,h,b}N).is_err()`. It **FAILS** against the current
implementation (`ret d0 → Ok(0xD65F0000)`). Once validation is added the property
passes; no assertion changes are needed.

## Fix

Reject FP/SIMD register names before/inside the operand parse:

```rust
if let Some(Operand::Reg(name)) = operands.get(0) {
    let c = name.chars().next().unwrap_or(' ').to_ascii_lowercase();
    if matches!(c, 'd' | 's' | 'q' | 'v' | 'h' | 'b') {
        return Err("ret requires a general-purpose register (Xn)".into());
    }
}
```
