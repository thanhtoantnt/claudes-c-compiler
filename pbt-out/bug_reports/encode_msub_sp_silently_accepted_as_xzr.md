# Bug Report: `encode_msub` silently accepts SP (encodes it as XZR)

**Location:** `src/backend/arm/assembler/encoder/data_processing.rs`, function `encode_msub`

## Summary

`encode_msub` resolves every operand through the shared `get_reg` helper, which
maps the stack-pointer name `"sp"` (and `"wsp"`) to register number **31** — the
encoding slot for the **zero register (XZR/WZR)**. For the data-processing
(3-source) class, ARMv8 defines register 31 as XZR, *not* SP. As a result,
`msub` instructions that name SP in any operand are accepted and silently
re-encoded as the zero register, producing a **different instruction** with no
diagnostic.

This is the same root cause already characterized (but not fixed) for the
sibling encoders `encode_mul` / `encode_madd` in this same file.

## Reproduction

No PBT property fails — the field-placement, differential-vs-MADD, width,
determinism, and error-contract properties all pass (`PROPTEST_CASES=2000`).
The finding was surfaced by code analysis of `get_reg`:

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    match name.as_str() {
        "sp" | "wsp" => Some(31),   // <-- SP and XZR share slot 31
        ...
```

```rust
pub(crate) fn encode_msub(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;
    let (rm, _) = get_reg(operands, 2)?;
    let (ra, _) = get_reg(operands, 3)?;   // no SP rejection anywhere
    ...
```

Minimal input:

```text
msub x0, x1, x2, sp
```

This is encoded as `msub x0, x1, x2, xzr`, i.e. the *alias* `mneg x0, x1, x2`
(`Rd = -Rn*Rm` instead of `Rd = SP - Rn*Rm`). The SP operand is lost. The same
applies to SP in Rd, Rn, or Rm.

## Impact

Silent miscompilation with a register-class error. An assembler that should
reject `msub ..., sp` (ARMv8: such encodings are UNPREDICTABLE/CONSTRAINED
UNPREDICTABLE for this instruction class) instead emits a semantically
distinct instruction. This is especially dangerous because MSUB with Ra=SP is
a plausible hand-written idiom for subtracting a product from the stack
pointer.

## Suggested fix

The data-processing (3-source) instructions permit only X0–X30 or XZR in
every operand position. Reject SP/WSP in `encode_msub` (and consistently in
`encode_madd`, `encode_mneg`, `encode_smaddl`, `encode_umaddl`, `encode_mul`,
`encode_smull`, `encode_umull`, `encode_smulh`, `encode_umulh`):

```rust
fn reject_sp(operands: &[Operand]) -> Result<(), String> {
    for (i, o) in operands.iter().enumerate() {
        if let Operand::Reg(n) = o {
            let lo = n.to_lowercase();
            if lo == "sp" || lo == "wsp" {
                return Err(format!("SP not permitted in operand {} of msub", i));
            }
        }
    }
    Ok(())
}
```

Call it at the top of `encode_msub` before `get_reg`.

## Test coverage added

Five `proptest!` properties added to the `tests` module of
`data_processing.rs` (all passing):

| Property | Oracle |
|---|---|
| `msub_full_field_placement` | Reference (ARMv8 fixed-format field layout, 0..=31) |
| `msub_differs_from_madd_only_in_bit15` | Differential vs `encode_madd` |
| `msub_sf_tracks_rd_width` | Width contract (sf bit) |
| `msub_is_deterministic` | Purity |
| `msub_missing_operands_return_err` | Negative/error contract (< 4 operands → Err) |
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/69
