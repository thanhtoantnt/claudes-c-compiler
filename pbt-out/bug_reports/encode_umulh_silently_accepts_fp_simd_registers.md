# BUG: `encode_umulh` silently accepts FP/SIMD register operands

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` — `encode_umulh`
**Severity:** Medium (silent mis-assembly: wrong register file, no diagnostic)

## Summary

`encode_umulh` delegates all register parsing to `get_reg` → `parse_reg_num`.
`parse_reg_num` accepts **every** register prefix (`x | w | d | s | q | v | h | b`)
and only caps the numeric value at 31. It performs **no register-FILE check**.
Consequently an input such as

```
umulh d0, d1, d2
```

is accepted and silently re-encoded as an integer multiply-high whose `Rd/Rn/Rm`
fields simply hold the numeric portion of the FP/SIMD names. The emitted word is
**bit-for-bit identical** to `umulh x0, x1, x2`:

```
umulh d0, d1, d2  -> 0x9bc27c20   (== umulh x0, x1, x2)
```

## Specification

Per the ARMv8 Architecture Reference Manual, **UMULH** (Unsigned Multiply High)
operates exclusively on 64-bit **general-purpose** registers:

```
UMULH <Xd>, <Xn>, <Xm>      ; Xd, Xn, Xm ∈ {X0..X30, XZR}
```

Floating-point / SIMD registers (`D/S/Q/V/H/B`) belong to a different
architectural register file and are **never** valid operands for UMULH. A
correct assembler must reject them with an error (e.g. GAS emits
`Error: operand mismatch -- 'd0'`) rather than emit a word that aliases a GPR.

This is the same defect class already documented for `encode_br`,
`encode_blr`, `encode_ret`, `encode_csel`, `encode_cinc`, `encode_cinv`,
`encode_cneg`, `encode_csinc`, `encode_csinv`, `encode_csneg`, etc. The
root cause is shared: the encoder layer trusts `parse_reg_num` and never
re-asserts the operand register class.

## Why this is distinct from the known W-width bug

`umulh_accepts_32bit_w_registers.md` documents that a `W` register (a valid
integer GPR of the *wrong width*) is silently accepted. A `D/V/S` register is a
stronger violation: it is not an integer register at all — it is the wrong
register *file*. Both stem from the same root cause (no operand validation in the
encoder) but cover independent input classes.

## Reproduction

```rust
use crate::backend::arm::assembler::parser::Operand;
use crate::backend::arm::assembler::encoder::data_processing::encode_umulh;

// umulh d0, d1, d2  -> should be Err, currently returns Ok(0x9bc27c20)
let ops = vec![
    Operand::Reg("d0".into()),
    Operand::Reg("d1".into()),
    Operand::Reg("d2".into()),
];
assert!(encode_umulh(&ops).is_err());     // FAILS today

// Silent aliasing evidence: identical word to the X form.
let fpsimd = encode_umulh(&ops).unwrap();
let gpr    = encode_umulh(&vec![
    Operand::Reg("x0".into()),
    Operand::Reg("x1".into()),
    Operand::Reg("x2".into()),
]).unwrap();
assert_eq!(fpsimd, gpr);                   // PASSES today (the bug)
```

Minimal failing input from proptest: `pos = 0, prefix_idx = 0, num = 0`
(i.e. `"d0"` placed in the destination position), `successes: 0`.

## Suggested fix

Validate the register class inside `encode_umulh` (or, better, in `get_reg` via
a class-restricting variant), rejecting any operand whose prefix is not `x`
(or the `xzr`/`sp` aliases, subject to the separate SP-vs-XZR finding). For
example:

```rust
fn require_x_reg(operands: &[Operand], idx: usize) -> Result<u32, String> {
    match operands.get(idx) {
        Some(Operand::Reg(name)) if is_64bit_reg(name) => {
            parse_reg_num(name).ok_or_else(|| format!("invalid register: {}", name))
        }
        other => Err(format!("expected 64-bit X register at operand {}, got {:?}", idx, other)),
    }
}
```

After the fix, `umulh_rejects_fp_simd_register_in_any_position` should pass; the
pinning property `umulh_sp_operand_silently_becomes_xzr` should be revisited
together with the broader SP-as-XZR handling.
