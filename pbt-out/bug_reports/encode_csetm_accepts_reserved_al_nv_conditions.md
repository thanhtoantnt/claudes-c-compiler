# BUG: `encode_csetm` accepts reserved `AL`/`NV` conditions and emits an undefined encoding

**Status:** CONFIRMED by property-based test and by the reference assembler llvm-mc-18.
**Severity:** Medium (emits an architecturally-rejected / disassembler-undefined word with no diagnostic).
**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_csetm`

## Summary

`CSETM <Rd>, <cond>` is an alias of `CSINV <Rd>, <XZR>, <XZR>, invert(<cond>)`.
Per the ARM ARM (C6.2.44) the aliased CSINV condition must **not** be `AL`
(`1110`) or `NV` (`1111`) — those are reserved condition-field values for the
conditional-select group. Because `invert` swaps `AL <-> NV`, a user-facing
`csetm Rd, al` or `csetm Rd, nv` produces a CSINV whose condition field is
`NV` / `AL`: a reserved encoding. Reference assemblers reject these:

```
$ echo 'csetm x0, al' | llvm-mc-18 -triple=aarch64 -show-encoding
<stdin>:1:13: error: condition codes AL and NV are invalid for this instruction
$ echo 'csetm x0, nv' | llvm-mc-18 -triple=aarch64 -show-encoding
<stdin>:1:13: error: condition codes AL and NV are invalid for this instruction
```

`encode_csetm` performs **no** such check and happily emits the reserved word.

## Reproducer

Minimal failing input found by `prop_rejects_al_nv_conditions` (in
`prop_encode_csetm_tests`):

```
case = 0      # i.e. csetm x0, al
```

```rust
let ops = vec![Operand::Reg("x0".into()), Operand::Cond("al".into())];
assert_eq!(
    encode_csetm(&ops),
    Ok(EncodeResult::Word(0xDA9FF3E0))   // silently encoded!
);
// 0xDA9FF3E0 == sf(1) op(1) S(0) 11010100 Rm(=11111) inv_cond(=1111=NV)
//               o2(0) o1(0) Rn(=11111) Rd(=0)
//                              ^^^^^^^^^ cond field == 1111 (NV) — RESERVED.
```

The expected behaviour is `Err(..)`. `csetm x0, nv` is likewise accepted (cond
field becomes `1110` = `AL`, also reserved).

## Root cause

`encode_csetm` computes `inv_cond = cond ^ 1` and places it in `[15:12]`
without ever testing `cond` (or `inv_cond`) against the reserved values:

```rust
pub(crate) fn encode_csetm(operands: &[Operand]) -> Result<EncodeResult, String> {
    // CSETM Rd, cond -> CSINV Rd, XZR, XZR, invert(cond)
    let (rd, is_64) = get_reg(operands, 0)?;
    let cond = match operands.get(1) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csetm requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let inv_cond = cond ^ 1;
    let word = (((sf << 31) | (1 << 30)) | (0b11010100 << 21)
        | (0b11111 << 16) | (inv_cond << 12)) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))   // no validation that cond != AL/NV
}
```

## Suggested fix

Reject `AL`/`NV` after decoding the condition:

```rust
let cond = match operands.get(1) {
    Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
    _ => return Err("csetm requires condition".to_string()),
};
// CSETM is an alias of CSINV; the aliased condition must not be AL (14) or
// NV (15) — invert swaps the two, so reject both up front.
if matches!(cond, 14 | 15) {
    return Err(format!("csetm: condition code AL/NV is invalid for this instruction"));
}
```

## Test status

`prop_encode_csetm_tests` (7 properties):

- `prop_opcode_structure_and_fields` ...... PASS
- `prop_csetm_equals_csinv_xzr_alias` ... PASS  (differential vs `encode_csinv`)
- `prop_cond_inverted_and_aliases` ....... PASS
- `prop_csetm_xor_cset_is_bits30_and_10`. PASS  (differential vs `encode_cset`)
- `prop_rejects_invalid_operands` ........ PASS
- `prop_rejects_fp_simd_registers` ....... **FAIL** (see
  `encode_csetm_silent_fp_simd_destination.md`)
- `prop_rejects_al_nv_conditions` ........ **FAIL** (this bug, CSETM-specific)

Happy-path correctness cross-checked by manual decode of the emitted word
(e.g. `csetm x0, eq` -> `0xDA9F17E0` == `CSINV x0, xzr, xzr, ne`,
sf=1 op=1 o1=0 Rm=Rn=31 cond=1=invert(eq)=ne Rd=0).

## Regression property

Failing property: `prop_rejects_al_nv_conditions`

```rust
prop_assert!(encode_csetm(&[xreg(rd)], "al").is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/31
