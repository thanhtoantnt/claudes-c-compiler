# Bug — `encode_fp_1src`: no FP-bank / precision-homogeneity validation

## Target
`pub(crate) fn encode_fp_1src(operands: &[Operand], opcode: u32) -> Result<EncodeResult, String>`
in `src/backend/arm/assembler/encoder/fp_scalar.rs`

## Bug
`ftype` is derived **only** from the destination register prefix:
```rust
let rd_name = match &operands[0] { Operand::Reg(r) => r.to_lowercase(), _ => String::new() };
let is_double = rd_name.starts_with('d');
let ftype = if is_double { 0b01u32 } else { 0b00 };
```
The function never verifies that:
- the **source** register matches the destination's precision, or
- the operands are FP registers at all.

ARMv8-A FRINT* require homogeneous S/D (or H under FP16) FP-register operands.
`get_reg` validates the register *number* range but not its *bank*.

Consequently all of these are accepted and emit a (wrong) word:
- `FRINTN D0, S1` → mixed precision; `ftype=01` taken from dest only.
- `FRINTN X0, X1` → GP registers; `ftype=00`; bogus FP word.
- `FRINTN H0, H1` → half-precision silently treated as single (`ftype=00`).

## Reproduction
Failing property test in `fp_scalar::tests`:
`prop_fp_1src_rejects_mismatched_precision_and_bank` — minimal input `n = 0`,
first failing clause `D0, S1`.
```
cargo test prop_fp_1src_rejects_mismatched_precision_and_bank
```

## Suggested fix
Validate both operands are FP registers of the same precision (e.g. via
`is_fp_reg` + comparing dest/source prefixes) and return `Err` otherwise. Map
half-precision (`H`) to `ftype=11` rather than silently defaulting to single.

## Severity
Medium (wrong-code on malformed operands; assembler must reject these forms).
