# BUG: `encode_scvtf` silently accepts wrong register banks (GP dest / FP source)

**Target:** `src/backend/arm/assembler/encoder/fp_scalar.rs` → `encode_scvtf`
→ `encode_int_to_float`
**Severity:** High (silent mis-encoding / wrong code generation)
**Status:** Confirmed failing — `prop_scvtf_rejects_wrong_operand_banks`

## Summary

`encode_scvtf` (and the shared helper `encode_int_to_float` it delegates to)
never validates the **register class** of either operand. Per ARMv8-A, `SCVTF`
converts a **GP integer source** (`Wn`/`Xn`) to an **FP destination**
(`Sd`/`Dd`/`Hd`). Any other bank combination is illegal and must be rejected.
Instead, the encoder derives `ftype` and `sf` from the operand name prefixes
(`'d'`/`'s'` → ftype, `'x'` → sf) with no bank check, so it silently accepts
illegal operands and emits a word that **collides with a legal instruction**.

## Spec basis (ARMv8-A FP–integer conversion)

`SCVTF <FPd>, <GPn>` — `sf 00 11110 ftype 1 00 010 000000 Rn Rd`
- `Rd` field = FP destination register (`Sd`/`Dd`/`Hd`).
- `Rn` field = GP integer source register (`Wn`/`Xn`).
- There is **no** `SCVTF Wd, Wn` or `SCVTF Dd, Dn` form.

## Reproduction (minimal, n=0)

```text
encode_scvtf(["w0", "w0"])   == Ok(Word(0x1E220000))   // GP DEST — illegal
encode_scvtf(["d0", "d0"])   == Ok(Word(0x1E620000))   // FP SOURCE — illegal

// Legal encodings the above collide with:
encode_scvtf(["s0", "w0"])   == Ok(Word(0x1E220000))   // == SCVTF S0,W0  (collision!)
encode_scvtf(["d0", "w0"])   == Ok(Word(0x1E620000))   // == SCVTF D0,W0  (collision!)
```

`encode_scvtf(["w0","w0"])` and `encode_scvtf(["s0","w0"])` return **identical**
words (`0x1E220000`), because `ftype` is mis-derived as `00` from the `'w'`
prefix. The illegal operand string is accepted and produces the encoding of a
*different*, valid instruction.

## Root cause

In `encode_int_to_float`:

```rust
let (rd, _) = get_reg(operands, 0)?;
let (rn, rn_is_64) = get_reg(operands, 1)?;

let dst_name = match &operands[0] {
    Operand::Reg(name) => name.to_lowercase(),
    _ => return Err("scvtf/ucvtf: expected register dest".to_string()),
};
let ftype: u32 = if dst_name.starts_with('d') { 0b01 } else { 0b00 }; // dest prefix only
let sf: u32 = if rn_is_64 { 1 } else { 0 };                           // source width only
```

- `ftype` is taken from the dest prefix; a `'w'` dest is treated as `00` (single)
  instead of rejected.
- `sf` comes from `rn_is_64` (whether the source name parses as 64-bit); a `'d'`
  source parses to 32-bit → `sf=0`, hiding the wrong bank.
- No check that the destination is in the FP bank or the source is in the GP bank.

## Impact

- Silent mis-encoding: illegal `SCVTF Wd,Wn` / `SCVTF Dd,Dn` produce *valid*
  words for *different* instructions. A typo or upstream parser slip yields
  wrong machine code with no error.
- The dest/source register *numbers* are still placed in `Rd`/`Rn`, so the
  emitted word is a plausible but semantically wrong `SCVTF`.

## Suggested fix

Validate operand banks in `encode_int_to_float` before encoding:

```rust
// dest must be FP (s/d/h), source must be GP (w/x)
let dst_is_fp = matches!(dst_name.chars().next(), Some('s') | Some('d') | Some('h'));
if !dst_is_fp {
    return Err(format!("scvtf/ucvtf dest must be an FP register, got {}", dst_name));
}
let src_name = match &operands[1] {
    Operand::Reg(name) => name.to_lowercase(),
    _ => return Err("scvtf/ucvtf: expected register source".to_string()),
};
if !matches!(src_name.chars().next(), Some('w') | Some('x')) {
    return Err(format!("scvtf/ucvtf source must be a GP register, got {}", src_name));
}
```

## Regression Property

Failing property: `prop_scvtf_rejects_wrong_operand_banks`

```rust
prop_assert!(encode_scvtf(&[wreg(0), wreg(0)]).is_err());  // GP dest
prop_assert!(encode_scvtf(&[dreg(0), dreg(0)]).is_err());  // FP source
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/150
