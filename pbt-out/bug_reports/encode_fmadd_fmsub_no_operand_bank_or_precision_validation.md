# Bug report — `encode_fmadd_fmsub`

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs`, `encode_fmadd_fmsub(operands, is_sub)`

## Summary

`encode_fmadd_fmsub` does not validate that its four operands are floating-point
registers, nor that they share a single precision. It derives `ftype` **only**
from `operands[0]`'s prefix and ORs every operand's register number straight into
the word. Consequently it silently accepts illegal operands and, in the GP case,
produces an instruction that collides with a completely different, valid mnemonic.

## Spec (ARMv8-A, "Floating-point data-processing (3 source)")

`FMADD/FMSUB <Fd>, <Fn>, <Fm>, <Fa>` requires all four operands to be FP/SIMD
registers and **all of the same precision** (all `S` or all `D`). Integer (W/X)
registers are not valid operands for any FP data-processing instruction.

## Findings (both reproduced by `prop_fmadd_rejects_mixed_precision_and_gp_bank`)

### 1. Mixed-precision operands are silently accepted

Minimal failing input: `n = 0`.

```
FMADD D0, S0, S0, S0  ->  Ok(EncodeResult::Word(0x1F400000))
```

`0x1F400000` is exactly `FMADD D0, D0, D0, D0`. The assembler silently rewrites a
mixed-precision source (illegal per the ARM ARM) into the all-double form with no
diagnostic. An S-register source is treated as if it were a D-register.

### 2. GP-bank (W/X) operands are silently accepted and collide with FP encoding

```
FMADD X0, X0, X0, X0  ->  Ok(EncodeResult::Word(0x1F000000))   // == FMADD S0, S0, S0, S0
FMADD W0, W0, W0, W0  ->  Ok(EncodeResult::Word(0x1F000000))   // identical
```

A floating-point fused-multiply-add instruction silently consumes integer
registers. `"x0"` does not start with `'d'`, so `ftype` falls through to `0b00`
(single) and the integer register numbers are packed into the Rd/Rn/Rm/Ra fields
just like `S0..`. The emitted word is bit-for-bit identical to `FMADD S0,S0,S0,S0`
— an unambiguous, no-diagnostic mis-assembly.

## Root cause

In `encode_fmadd_fmsub`:

```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
```

`ftype` is derived only from the destination. The function:

- never calls `is_fp_reg()` on `operands[0..4]` (GP registers pass straight through `get_reg` → `parse_reg_num`);
- never checks that `operands[1..3]` share the destination's precision prefix.

(`get_reg`/`parse_reg_num` *do* correctly reject register numbers > 31 — that
contract holds; the problem is bank/precision validation, which is entirely
absent. `encode_fnmadd_fnmsub`, right next to it, shares the identical defect.)

## Suggested fix

Before encoding, validate:
1. every one of `operands[0..4]` is an FP register (`is_fp_reg`);
2. all four share the destination precision (`'s'`/`'d'`), returning `Err` on any
   mismatch — mirroring the validation already present in the FP-arith / FMOV paths
   elsewhere in this encoder.

## Test status

`cargo test --lib fp_scalar::tests::prop_fmadd` → **4 passed, 1 failed**:

| Property | Result |
|---|---|
| `prop_fmadd_places_fields` | ok |
| `prop_fmadd_ftype_and_o1_derivation` | ok |
| `prop_fmadd_is_deterministic` | ok |
| `prop_fmadd_rejects_bad_regs_arity` | ok |
| `prop_fmadd_rejects_mixed_precision_and_gp_bank` | **FAILS (this finding)** |
