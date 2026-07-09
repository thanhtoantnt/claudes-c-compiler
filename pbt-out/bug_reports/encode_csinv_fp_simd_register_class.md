# Bug Report: `encode_csinv` accepts FP/SIMD register operands

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_csinv`
(triggered by `parse_reg_num` in `src/backend/arm/assembler/encoder/mod.rs:131`)

## Summary

`encode_csinv` silently accepts FP/SIMD register names (`d`/`s`/`q`/`v`/`h`/`b`) in
any register slot and re-encodes them with their numeric index as if they were
general-purpose (X/W) registers. CSINV is defined ONLY on general-purpose
registers (ARMv8 ARM, conditional-select group), so an FP/SIMD operand is
architecturally invalid and must be rejected.

## Minimal input

```
ops = [Reg("b0"), Reg("x1"), Reg("x2"), Cond("eq")]   // mnemonic: csinv b0, x1, x2, eq
```

## Expected vs actual

- **Expected:** `Err` (FP/SIMD register not valid for a GP conditional-select).
- **Actual:** `Ok(Word(0x5A820020))` (i.e. `1518469152`). `b0` is parsed as
  register number `0` with `sf=0`, emitting a GP CSINV with `Rd=0, Rn=1, Rm=2,
  cond=eq`.

**Witness (shrunk counterexample):** surfaced by the failing property
`prop_encode_csinv_tests::prop_rejects_fp_simd_registers`.
reproduce: `prefix="b", n=0, slot=0` → `Ok(Word(1518469152))`.

## Impact

- A GP instruction word is emitted for an FP/SIMD-sourced CSINV; the operand's
  register class is silently lost and the output cannot be distinguished from a
  legitimate GP encoding.
- Affects all three register slots (Rd, Rn, Rm), since each flows through the same
  permissive `parse_reg_num` path.

## Root cause

`parse_reg_num` (encoder/mod.rs:131) accepts every register-bank prefix
(`x|w|d|s|q|v|h|b`), so FP/SIMD names parse successfully. `encode_csinv` then uses
the parsed number unconditionally and never validates that the register belongs to
the GP (X/W) class.

## Fix

Add a GP-class check for CSINV's register operands, e.g. by rejecting names whose
prefix is not `x`/`w`/`sp`/`wsp`/`xzr`/`wzr`/`lr` (mirroring the `is_64bit_reg` /
`is_32bit_reg` helpers in encoder/mod.rs:157/163), returning `Err` otherwise.
