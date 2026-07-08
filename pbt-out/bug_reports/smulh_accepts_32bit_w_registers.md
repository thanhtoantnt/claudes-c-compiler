# BUG: `encode_smulh` accepts 32-bit (W) register operands without validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` — `encode_smulh`
**Severity:** Medium (silent mis-assembly / architecturally UNDEF output)

## Summary

`encode_smulh` calls `get_reg(operands, i)` for each operand, which returns
`(reg_num, is_64)`, but it **discards the `is_64` flag** and never validates
that the operands are 64-bit (`X`) registers. As a result, an input such as

```
smulh w0, w1, w2
```

is accepted and silently re-encoded as a 64-bit instruction (`sf = 1`), instead
of being rejected.

## Specification

Per the ARMv8 Architecture Reference Manual, **SMULH** (Signed Multiply High)
is defined **only** as

```
SMULH Xd, Xn, Xm        ; Xd, Xn, Xm must be 64-bit (X) registers
```

There is **no 32-bit (W) form**. The encoding space `sf=0` for op31=010 is
UNDEFINED. Therefore a correct assembler must reject any W-register operand
with an error.

## Reproduction

```rust
// smulh w0, w1, w2  -> should be Err, currently returns Ok(0x9b407c00)
let ops = vec![Operand::Reg("w0".into()), Operand::Reg("w1".into()),
               Operand::Reg("w2".into())];
encode_smulh(&ops)   // Ok(Word(0x9b407c00))  -- BUG
```

The produced word `0x9b407c00` is identical to `smulh x0, x1, x2`, i.e. the
assembler silently "promotes" 32-bit register names to their 64-bit encoding.

## Root cause

```rust
pub(crate) fn encode_smulh(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;   // is_64 discarded
    let (rn, _) = get_reg(operands, 1)?;   // is_64 discarded
    let (rm, _) = get_reg(operands, 2)?;   // is_64 discarded
    let word = (1u32 << 31) | ...;          // sf hardcoded to 1, no width check
    Ok(EncodeResult::Word(word))
}
```

The hardcoded `sf = 1` is *correct* (SMULH is inherently 64-bit), but no check
guarantees the operands actually *are* `X` registers.

## Scope of the same bug

The identical pattern (discard `is_64`, no width validation) affects the whole
"long/high multiply" family, all of which have a fixed register-width contract:

| Instruction | Required widths          | Currently validated? |
|-------------|--------------------------|----------------------|
| `SMULH`     | `Xd, Xn, Xm` (all 64)    | No                   |
| `UMULH`     | `Xd, Xn, Xm` (all 64)    | No                   |
| `SMULL`     | `Xd, Wn, Wm`             | No (accepts X sources) |
| `UMULL`     | `Xd, Wn, Wm`             | No                   |
| `SMADDL`    | `Xd, Wn, Wm, Xa`         | No                   |
| `UMADDL`    | `Xd, Wn, Wm, Xa`         | No                   |

## Suggested fix

In `encode_smulh` (and `encode_umulh`), require `is_64 == true` for all three
operands, returning `Err` otherwise, e.g.:

```rust
let (rd, rd64) = get_reg(operands, 0)?;
let (rn, rn64) = get_reg(operands, 1)?;
let (rm, rm64) = get_reg(operands, 2)?;
if !(rd64 && rn64 && rm64) {
    return Err("smulh requires 64-bit (X) registers".to_string());
}
```

## Test evidence

```
smulh_fixed_fields .............................. ok
smulh_register_field_placement .................. ok
smulh_always_64bit .............................. ok
smulh_vs_umulh_only_sign_bit_differs ............ ok
smulh_rejects_32bit_w_registers ................. FAILED   <-- this bug
```
