# Bug Report: `encode_fmadd_fmsub` silently accepts illegal operands (no precision/bank validation)

## Location
`src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_fmadd_fmsub(operands, is_sub)`

## Summary
`encode_fmadd_fmsub` derives the `ftype` (precision) field **solely from `operands[0]`** (the destination) and never validates:
1. that any operand is a floating-point register (the GP-bank registers `Wn`/`Xn` are silently accepted), or
2. that all four FP operands share the same precision (`S` vs `D`).

Per the ARMv8-A architecture reference manual, `FMADD`/`FMSUB` operate **only on FP/SIMD registers** and require **homogeneous precision** across all four operands (`Rd, Rn, Rm, Ra`). Mixed precision and GP-bank operands are illegal inputs that should be rejected with an `Err`, not silently mis-encoded.

## Reproduction (minimal failing input)
Property `prop_fmadd_rejects_mixed_precision_and_gp_bank` with `n = 0`:

```rust
// Mixed precision: dest double, sources single.
let ops = vec![
    Operand::Reg("d0".into()),  // dest
    Operand::Reg("s0".into()),  // rn  (single!)
    Operand::Reg("s0".into()),  // rm  (single!)
    Operand::Reg("s0".into()),  // ra  (single!)
];
encode_fmadd_fmsub(&ops, false)  // -> Ok(Word(524288000))   == 0x1F400000
```

The result `0x1F400000` is the encoding of `FMADD D0, D0, D0, D0` — the `ftype=01` (double) was taken from the `d0` destination, while the three single-precision `s0` sources were silently treated as double-precision. The emitted instruction does not match the assembler input.

## Root cause
```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
```
Only `operands[0]` is inspected. No prefix check on `operands[1..4]`, no `is_fp_reg` check on any operand. GP registers (`w`/`x`) fall through to `ftype = 0b00` (single) and are encoded as if they were FP registers.

## Impact
- **Miscompilation / wrong machine code:** the emitted word does not correspond to the textual instruction, producing silently incorrect object code for any `fmadd`/`fmsub` using GP registers or mismatched FP widths.
- **Confused diagnostics:** an obviously malformed input (e.g. `fmadd x0, x1, x2, x3`) is accepted instead of being reported as an error.

## Suggested fix
After parsing the four registers, validate:
- every operand is an FP/SIMD register (`is_fp_reg`), and
- all four share the same precision prefix (`d` vs `s`); mismatch ⇒ `Err`.

```rust
let prefixes = [operands[0], operands[1], operands[2], operands[3]]
    .iter()
    .map(|o| match o { Operand::Reg(r) => r.to_lowercase(), _ => String::new() })
    .collect::<Vec<_>>();
if !prefixes.iter().all(|n| is_fp_reg(n)) {
    return Err("fmadd/fmsub requires floating-point register operands".to_string());
}
let is_double = prefixes.iter().all(|n| n.starts_with('d'));
let all_single = prefixes.iter().all(|n| n.starts_with('s'));
if !(is_double || all_single) {
    return Err("fmadd/fmsub operands must all be the same precision (all S or all D)".to_string());
}
```

## Verification
Property suite added to `fp_scalar.rs` (`mod tests`):
- `prop_fmadd_places_fields` — PASS (reference/field layout oracle)
- `prop_fmadd_ftype_and_o1_derivation` — PASS (precision + FMADD/FMSUB differ only in bit 15)
- `prop_fmadd_is_deterministic` — PASS
- `prop_fmadd_rejects_bad_regs_arity` — PASS (out-of-range registers and arity are correctly rejected via `get_reg`/`parse_reg_num`)
- `prop_fmadd_rejects_mixed_precision_and_gp_bank` — **FAIL** (this bug)

Baseline (correct) encoding confirmed against the ARM ARM:
`FMADD D0,D0,D0,D0 = 0x1F400000`, `FMADD S0,S0,S0,S0 = 0x1F000000`, `FMSUB D0,D0,D0,D0 = 0x1F408000`.
