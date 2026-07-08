# Bug Report — `encode_cneg` silently accepts FP/SIMD register operands

## Target
`src/backend/arm/assembler/encoder/compare_branch.rs` — `encode_cneg`
(alias of CSNEG: `CNEG Rd, Rn, cond -> CSNEG Rd, Rn, Rn, invert(cond)`)

## Summary
`encode_cneg` accepts floating-point / SIMD register names (`d`, `s`, `q`, `v`,
`h`, `b`) in the `Rd` and `Rn` operand slots and silently re-encodes them as
general-purpose registers, using the FP register's numeric index in the GP
register field. Per the ARM ARM (C4.1.67, "Conditional Select (negate)"),
CSNEG/CNEG are defined **only** on general-purpose (X/W) registers, so these
operands should be rejected with `Err`, not encoded.

## Root cause
`encode_cneg` resolves its register operands via the shared helper `get_reg`
(`src/backend/arm/assembler/encoder/mod.rs:956`), which calls `parse_reg_num`
(`mod.rs:131`). `parse_reg_num` treats the prefixes `d | s | q | v | h | b`
exactly like `x | w`, returning `Some(index)` for any `0..=31`. `is_64bit_reg`
then returns `false` for an FP prefix (so `sf` is forced to 0 / 32-bit), and
the index is placed directly into the Rd/Rn field. The FP/SIMD class check
`is_fp_reg` (`mod.rs:166`) exists but is never consulted here.

## Reproduction
Property `prop_encode_cneg_tests::prop_rejects_fp_simd_registers` fails:

```
cargo test --lib prop_encode_cneg_tests::prop_rejects_fp_simd_registers
...
panicked: encode_cneg should reject FP/SIMD register in slot 0
          (got Ok(Word(1518408736)))
minimal failing input: prefix = "b", n = 0, slot = 0
```

Concrete encoding: `cneg b0, x1, eq` is accepted and yields
`Ok(Word(0x5A800000))` — a valid-looking CSNEG with sf=0, op=1, o1=1, Rd=0,
Rn=1, cond=inverted(`eq`)=`ne`. The `b0` operand was silently treated as
`Rd = x0` (numeric index 0).

## Impact
- An assembler source that mistakenly (or via typo) names an FP/SIMD register
  in a CNEG operand is mis-assembled into a *different, valid* instruction
  operating on a GP register, with no diagnostic — the classic "wrong
  instruction, no error" failure mode.
- The same defect affects the entire conditional-select family that shares
  `get_reg`: `encode_csel`, `encode_csinc`, `encode_csinv`, `encode_csneg`,
  `encode_cneg`, `encode_cinc`, `encode_cinv` (each documented separately
  under `pbt-out/bug_reports/`).

## Suggested fix
Add a GP-register guard. A per-call-site helper is safer than editing `get_reg`
globally, because load-store and FP/SIMD encoders legitimately accept FP
registers:

```rust
fn get_gp_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    match operands.get(idx) {
        Some(Operand::Reg(name)) => {
            if is_fp_reg(name) {
                return Err(format!("expected GP register, got FP/SIMD: {}", name));
            }
            let num = parse_reg_num(name).ok_or_else(|| format!("invalid register: {}", name))?;
            Ok((num, is_64bit_reg(name)))
        }
        other => Err(format!("expected register at operand {}, got {:?}", idx, other)),
    }
}
```
Routing the conditional-select encoders (`csel`/`csinc`/`csinv`/`csneg`/
`cneg`/`cinc`/`cinv`) through this variant fixes all of them at once.

## Verification
5/6 new CNEG properties **pass**:
- `prop_opcode_structure_and_fields` — fixed opcode/field placement + full word
  reconstruction (incl. CNEG-specific o2:o1 == `01` and the Rm==Rn alias
  collapse).
- `prop_cneg_equals_csneg_rn_rn_inverted` — defining differential:
  `cneg Rd,Rn,cond == csneg Rd,Rn,Rn,invert(cond)` for every cond incl. al/nv.
- `prop_cneg_xor_cinv_is_bit10` — CNEG vs CINV differ only in bit 10 (o1).
- `prop_sf_bit_is_bit31` — sf derived from Rd only.
- `prop_rejects_invalid_operands` — operand-shape negative contract.

1/6 **fails** as documented above — the failure *is* the proof of the bug.

## Regression property

Failing property: `prop_rejects_fp_simd_registers`

```rust
prop_assert!(encode_cneg(&[xreg(rd), xreg(rn)], "eq").is_err());
```
