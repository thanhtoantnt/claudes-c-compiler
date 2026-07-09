# Bug: `encode_int_to_float` does not validate operand register banks

**File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
**Function:** `encode_int_to_float(operands, is_signed)` (also reached via `encode_scvtf` / `encode_ucvtf`)
**Severity:** Medium — silent mis-encoding of illegal instructions (no error, wrong machine code)

## Summary

`encode_int_to_float` encodes the ARMv8-A `SCVTF`/`UCVTF` instructions, which convert a
**GP integer** source (`Wn`/`Xn`) into an **FP** destination (`Sd`/`Dd`). The function never
validates that the source is a GP register or that the destination is an FP register. It only
inspects the dest prefix (for `ftype`) and the source width flag (for `sf`), so illegal bank
combinations are silently accepted and produce a machine word with the wrong register-class
semantics.

## Root cause

In `encode_int_to_float`:

```rust
let (rd, _) = get_reg(operands, 0)?;            // dest: NO bank check
let (rn, rn_is_64) = get_reg(operands, 1)?;      // source: NO bank check
...
let ftype: u32 = if dst_name.starts_with('d') { 0b01 } else { 0b00 };
let sf: u32 = if rn_is_64 { 1 } else { 0 };      // is_64bit_reg only true for 'x'/'sp'/...
```

- `get_reg` (in `encoder/mod.rs`) accepts **any** register-bank prefix (`x/w/d/s/q/v/h/b`) and
  only validates the numeric range (0–31).
- `is_64bit_reg` returns `true` only for `x`/`sp`/`xzr`/`lr`. For an FP source like `d3` it
  returns `false`, so `sf` is silently set to `0` (32-bit source) — wrong.
- `ftype` is `01` only when the dest starts with `d`; *every other prefix* (`w`, `x`, `q`, …)
  yields `ftype = 00` (single), so a GP dest is silently treated as `Sd`.

## Evidence (property test)

`prop_int_to_float_rejects_wrong_operand_banks` fails on the minimal input `n = 0`:

```
GP destination (w0) must be rejected; SCVTF/UCVTF dest must be FP, got Ok(Word(505544704))
```

`505544704 == 0x1E220000`, which is **exactly** the encoding of the valid instruction
`SCVTF S0, W0`. So `SCVTF W0, W0` (GP dest) emits the same bytes as `SCVTF S0, W0` — a real
toolchain (e.g. `llvm-mc`) rejects `SCVTF Wd, Wn` as an invalid operand.

Likewise an FP source is mis-encoded: `SCVTF D0, D3` is accepted with `sf = 0` (because `d`
is not a 64-bit GP register), producing `0x1E260000` — the encoding for `SCVTF D0, W3`.

## Impact

- Assembler emits illegal/semantically-wrong instructions with no diagnostic.
- A GP destination silently collides with a valid `Sd` destination encoding (aliasing).
- An FP source silently collides with a 32-bit GP source (`W`) encoding.

## Suggested fix

After `get_reg`, validate the banks explicitly (helper `is_fp_reg` already exists in
`encoder/mod.rs`; add/use a GP-bank predicate):

```rust
if !is_fp_reg(&dst_name) {
    return Err("SCVTF/UCVTF: destination must be an FP register (S/D)".into());
}
let src_name = match &operands[1] {
    Operand::Reg(n) => n.to_lowercase(),
    _ => return Err("SCVTF/UCVTF: expected register source".into()),
};
if !(src_name.starts_with('w') || src_name.starts_with('x')) {
    return Err("SCVTF/UCVTF: source must be a GP register (W/X)".into());
}
```

## Properties added (src/backend/arm/assembler/encoder/fp_scalar.rs, `mod tests`)

| Property | Oracle | Result |
|---|---|---|
| `prop_int_to_float_places_fields` | reference / field layout | PASS |
| `prop_int_to_float_sf_ftype_derivation` | sf from source width, ftype from dest prefix | PASS |
| `prop_int_to_float_signedness_selects_opcode` | SCVTF=010 vs UCVTF=011 | PASS |
| `prop_int_to_float_rejects_bad_regs_and_arity` | range/arity/non-reg rejection | PASS |
| `prop_int_to_float_rejects_wrong_operand_banks` | negative contract (bank validation) | **FAIL** (this bug) |
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/130
