# Bug: `encode_fp_arith` derives `ftype` from destination only — no precision-consistency check

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_fp_arith(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String>`
**Severity:** Medium (latent — masked only because well-formed inputs share precision.)

## Summary
`ftype` is read **only** from `operands[0]`'s first character
(`'d'` ⇒ `0b01` double, else ⇒ `0b00` single). The precision of the source
operands (Rn, Rm) is never inspected, and no consistency is enforced. AArch64
scalar FP data-processing (2-source) requires the destination and both sources to
share precision; a mixed-precision word is **UNALLOCATED**. E.g.
`FADD D0, S1, S2` encodes with `ftype = 0b01` (double, from `D0`) while the
source fields carry single-precision register numbers — an invalid instruction
`as`/`llvm-mc` reject.

## Minimal failing input
```
encode_fp_arith(&[Reg("d0"), Reg("s1"), Reg("s2")], 0b0010)  // FADD D0, S1, S2
```
- **Expected:** `Err` (operands must share precision).
- **Actual:** `Ok(Word(0x1E608820))` — `ftype = 0b01` from the `D0` destination,
  source registers silently reinterpreted as double-precision slots.

Discovered by property `prop_fp_arith_rejects_wrong_banks_precision_and_oversized_opcode`
(mixed-precision sub-assertion; surface test reaches it after the GP-bank defect is fixed).

## Impact
Mismatched-precision operands yield UNALLOCATED encodings that real AArch64
hardware rejects, with no compiler diagnostic.

## Fix
After the bank check, require homogeneous precision across all three FP operands:

```rust
let prec: Vec<char> = operands.iter().take(3).filter_map(|o| match o {
    Operand::Reg(n) => n.to_lowercase().chars().next(),
    _ => None,
}).collect();
if !(prec.iter().all(|&c| c == 'd') || prec.iter().all(|&c| c == 's')) {
    return Err("fp_arith operands must share precision (all D or all S)".into());
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/147
