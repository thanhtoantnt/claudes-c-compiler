# BUG: `encode_csel` silently accepts FP/SIMD registers and re-encodes them as GP registers

**Status:** CONFIRMED by property-based test (1 of 5 `prop_encode_csel_tests` fails).
**Severity:** Medium (silent mis-encoding → wrong machine code, no diagnostic).
**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_csel`
**Also affects:** the entire conditional-select / compare family that shares `get_reg`
(`encode_csinc`, `encode_csinv`, `encode_csneg`, `encode_cset`, `encode_csetm`,
`encode_ccmp_ccmn`, and likely many other encoders that call `get_reg`).

## Summary

`encode_csel` validates only that operands 0–2 are `Operand::Reg`; it performs **no
register-class check**. Because `get_reg` → `parse_reg_num` happily maps FP/SIMD
register names (`b0`, `d1`, `s2`, `q3`, `h4`, `v5`) to a 5-bit number, any FP/SIMD
register is silently accepted and emitted as if it were a general-purpose (X/W)
register. The result is an AArch64 word that is **architecturally unallocated** for
CSEL (and, depending on the exact field values, may alias a different instruction).

Per the ARM ARM (C4.1.64, *Conditional Select*), `CSEL` is defined **only** on
general-purpose registers (`Rd`, `Rn`, `Rm` ∈ X/W). Reference assemblers reject FP
operands:

```
$ llvm-mc -triple=aarch64 -show-encoding <<< 'csel b0, x1, x2, eq'
error: operand 0 must be an integer register, ...
$ aarch64-linux-gnu-as   # "operand mismatch -- expected an integer register"
```

## Reproducer

Minimal failing input found by `prop_rejects_fp_simd_registers`:

```
prefix = "b", n = 0, slot = 0      # i.e. csel b0, x1, x2, eq
```

```rust
let ops = vec![
    Operand::Reg("b0".into()),   // <-- FP/SIMD register, should be rejected
    Operand::Reg("x1".into()),
    Operand::Reg("x2".into()),
    Operand::Cond("eq".into()),
];
assert_eq!(
    encode_csel(&ops),
    Ok(EncodeResult::Word(0x1A802020))   // silently encoded!
);
// 0x1A802020 == sf(0) op(0) S(0) 11010100 Rm(=2) cond(=eq) o2(0) o1(0) Rn(=1) Rd(=0)
// Rd=0 came straight from parse_reg_num("b0") == Some(0).
```

The expected behaviour is `Err(..)`. The same drift happens for `d/s/q/v/h/b` in any
of the three register slots.

## Root cause

`get_reg` (in `encoder/mod.rs`) calls `parse_reg_num`, whose prefix matcher treats
`x|w|d|s|q|v|h|b` uniformly and never records the register class:

```rust
fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
        let num: u32 = name[1..].parse().ok()?;
        if num <= 31 { Some(num) } else { None }
    }
    ...
}
```

`is_64bit_reg` then classifies anything not starting with `x` as 32-bit, so `b0`
even gets `sf = 0` rather than being rejected. There is no `is_gp_reg` guard
applied by `encode_csel` (or `get_reg`) before encoding.

## Suggested fix

Add a general-purpose-register guard to `get_reg` (preferred — fixes the whole
family) or, minimally, to `encode_csel` and its siblings:

```rust
fn is_gp_reg(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with('x') || n.starts_with('w')
        || matches!(n.as_str(), "sp" | "wsp" | "xzr" | "wzr" | "lr")
}
```

Then reject non-GP registers with an error such as
`"expected general-purpose register, got {name}"`.

## Test status

- `prop_opcode_structure_and_fields` .... PASS
- `prop_sf_bit_is_bit31` ................ PASS
- `prop_cond_round_trips_and_aliases` ... PASS
- `prop_rejects_invalid_operands` ...... PASS
- `prop_rejects_fp_simd_registers` ..... **FAIL** (this bug)

Once the fix lands, `prop_rejects_fp_simd_registers` will turn green and the
proptest regression seed recorded in
`proptest-regressions/backend/arm/assembler/encoder/compare_branch.txt` can be
deleted.

## Related observation (not asserted by a property, low severity)

`sf` is derived **only** from `Rd`. `csel x0, w1, w2, eq` and `csel x0, x1, x2, eq`
encode to **bit-identical** words — the encoder does not verify that `Rn`/`Rm`
share `Rd`'s width. A reference assembler rejects mixed widths (`operand size
mismatch`). Worth a follow-up `get_reg`-level width-consistency check, but out of
scope for the current failing test.

## Regression property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_csel(&[xreg(rd), xreg(rn), xreg(rm)], "eq").is_err());
```
