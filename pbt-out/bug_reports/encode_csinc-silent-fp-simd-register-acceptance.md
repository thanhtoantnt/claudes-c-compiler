# BUG: `encode_csinc` silently mis-encodes FP/SIMD register operands as GP registers

**Status:** CONFIRMED — reproducing property test added and failing.
**Severity:** High (silent wrong-code / mis-assembly).
**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs::encode_csinc`
**Root cause:** `src/backend/arm/assembler/encoder/mod.rs::get_reg` (line 956) + `parse_reg_num` (line 131).

## Summary

`encode_csinc` (and the entire A64 conditional-select family: `encode_csel`,
`encode_csinv`, `encode_csneg`) accept floating-point / SIMD register names
(`d`, `s`, `q`, `v`, `h`, `b`) in the `Rd`/`Rn`/`Rm` slots and **silently
encode them as if they were general-purpose (X/W) registers**, instead of
returning `Err`.

CSINC is defined by the ARM ARM (C4.1.66 "Conditional Select (increment)")
**only on general-purpose registers**. Feeding it a SIMD/FP register is
undefined; the assembler must reject it, not produce a plausible-looking word.

## Root cause

`parse_reg_num` (`mod.rs:131-146`) deliberately accepts every register-bank
prefix:

```rust
match prefix {
    'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
        let num: u32 = name[1..].parse().ok()?;
        if num <= 31 { Some(num) } else { None }
    }
    _ => None,
}
```

`get_reg` (`mod.rs:956-966`) calls `parse_reg_num` and computes `is_64` via
`is_64bit_reg`, but **never validates the register class**:

```rust
fn get_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    match operands.get(idx) {
        Some(Operand::Reg(name)) => {
            let num = parse_reg_num(name)
                .ok_or_else(|| format!("invalid register: {}", name))?;
            let is_64 = is_64bit_reg(name);      // <-- only width, not class
            Ok((num, is_64))
        }
        ...
```

Note that an `is_fp_reg` helper (`mod.rs:161`) already exists but is unused by
`get_reg`.

## Reproduction

```bash
cargo test --lib prop_encode_csinc_tests::prop_rejects_invalid_operands
```

Minimal failing input (case 10):

```rust
encode_csinc(&[Operand::Reg("d0".into()),
               Operand::Reg("x1".into()),
               Operand::Reg("x2".into()),
               Operand::Cond("eq".into())])
// => Ok(Word(0x1A80_0400))     // expected: Err(...)
```

`0x1A80_0400` decodes as `csinc w0, x1, x2, eq` — the FP register `d0` was
quietly turned into `w0` (`is_64 = false` because `'d'` is not `'x'`). The
assembler emits a valid-looking, but semantically wrong, instruction.

## Impact

* **Silent mis-assembly.** Any source like `csinc d0, x1, x2, eq` (a typo, or
  a bad codegen pass) assembles without error and produces `csinc w0, ...`.
* **Shared across the family.** The sibling CSEL test
  `prop_encode_csel_tests::prop_rejects_fp_simd_registers` fails identically
  (`prefix = "b", slot = 0`), confirming `get_reg` is the common cause. Every
  encoder that funnels register operands through `get_reg` inherits the bug
  (this file alone: `encode_csel`, `encode_csinc`, `encode_csinv`,
  `encode_csneg`, and the `cset/csetm/cinc/cinv/cneg` aliases).

## Suggested fix

In `get_reg`, reject non-GP register names (or route FP/SIMD through a
dedicated helper). The existing `is_fp_reg` can be reused:

```rust
fn get_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    match operands.get(idx) {
        Some(Operand::Reg(name)) => {
            if is_fp_reg(name) {
                return Err(format!("expected GP register, got FP/SIMD register: {}", name));
            }
            let num = parse_reg_num(name).ok_or_else(|| format!("invalid register: {}", name))?;
            Ok((num, is_64bit_reg(name)))
        }
        other => Err(format!("expected register at operand {}, got {:?}", idx, other)),
    }
}
```

(Caveat: a few encoders may intentionally accept the full register bank — e.g.
vector load/store or FP arithmetic. Add a `get_reg_gp` variant if a blanket
`get_reg` change is too broad, or audit call sites.)

## Verification

After the fix, all FP/SIMD cases (10–14) in
`prop_encode_csinc_tests::prop_rejects_invalid_operands` and the CSEL sibling
test should pass with no other test regressions.

## Regression property

Failing property: `prop_rejects_invalid_operands`

```rust
prop_assert!(encode_csinc(&[xreg(rd), xreg(rn), xreg(rm)], "eq").is_err());
```
