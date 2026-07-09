# BUG: `encode_fcmp` silently accepts illegal operands (mixed precision & GP bank)

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`, function `encode_fcmp`
**Found by:** property-based tests (`prop_fcmp_rejects_mismatched_precision_and_bank`)
**Severity:** correctness / assembler soundness — emits an instruction that does **not** match the source text.

## Summary

`encode_fcmp` derives the precision field `ftype` **only** from `operand[0]`'s name prefix
and never validates `operand[1]`. As a result two classes of illegal operands are accepted
without error and mis-encoded:

```rust
let rn_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rn_name.starts_with('d');                 // <-- only operand[0]
let ftype = if is_double { 0b01 } else { 0b00 };
...
let (rm, _) = get_reg(operands, 1)?;                      // <-- no bank/precision check
```

### 1. Mixed-precision operands

`FCMP D0, S1` -> `Ok(Word(0x1E612000))`. That word decodes to **`FCMP D0, D1`**:
the `S1` register number (1) is placed into the double-precision `Rm` field, so the
assembler silently re-typed the second operand from single to double.

Likewise `FCMP S0, D1` -> `Ok(0x1E212000)` decodes to `FCMP S0, S1` (double re-typed to single).

### 2. General-purpose (GP) bank operands

`FCMP W0, W1` and `FCMP X0, X1` are accepted. Because `'w'`/`'x'` do not start with `'d'`,
`ftype` is forced to `0b00` (single) and the GP register numbers are written into the FP
`Rn`/`Rm` fields, producing an encoding that operates on the **FP** registers `S0`/`S1`
while the source named GP registers `W0`/`X0`. This is a silent bank mismatch.

## Correct behavior (per ARMv8-A ARM, "FCMP - Floating-point quiet compare")

`FCMP` requires both operands to be FP registers of the **same** precision (`<Sn>,<Sm>` or
`<Dn>,<Dm>`), or the second operand to be the immediate `#0.0`. GP-bank registers and
mixed precision are assembly errors and **must** be rejected by the assembler (return `Err`).

## Property test result

`prop_fcmp_rejects_mismatched_precision_and_bank` fails on the minimal input `n = 0`:

```
Test failed: mixed precision (Dn, Sm) must be rejected; got Ok(Word(509616128))
  509616128 = 0x1E612000  ->  FCMP D0, D1   (operand[1] silently re-typed)
minimal failing input: n = 0
```

The other five `prop_fcmp_*` properties pass, confirming the correct encodings and field
layout for the *legal* cases (homogeneous `S`/`D` register form, both `#0.0` spellings,
determinism, precision derivation, and out-of-range/non-zero-imm rejection via `get_reg`).

## Suggested fix

After resolving both operands, assert that both register names share the same FP prefix
(`d`/`d` or `s`/`s`) and reject any non-FP bank prefix, e.g.:

```rust
let rn_name = ...;
let rm_name = match &operands[1] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let rn_is_d = rn_name.starts_with('d');
let rn_is_s = rn_name.starts_with('s');
let rm_is_d = rm_name.starts_with('d');
let rm_is_s = rm_name.starts_with('s');
if !((rn_is_d && rm_is_d) || (rn_is_s && rm_is_s)) {
    return Err(format!("FCMP requires same-precision FP operands, got {}, {}",
                       rn_name, rm_name));
}
```

(The existing `encode_fmov` already does analogous bank/precision validation and can serve
as a pattern; `encode_fp_arith` / `encode_fp_1src` share this same latent gap.)
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/131
