# BUG: `encode_cset` accepts reserved `AL`/`NV` conditions and emits an undefined encoding

**Status:** CONFIRMED by property-based test and by the reference assembler llvm-mc-18.
**Severity:** Medium (emits an architecturally-rejected / disassembler-undefined word with no diagnostic).
**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_cset`

## Summary

`CSET <Rd>, <cond>` is an alias of `CSINC <Rd>, <XZR>, <XZR>, invert(<cond>)`. Per
the ARM ARM (C6.2.43) the aliased CSINC condition must **not** be `AL` (`1110`) or
`NV` (`1111`) — those are reserved condition-field values for the conditional-select
group. Because `invert` swaps `AL <-> NV`, a user-facing `cset Rd, al` or
`cset Rd, nv` produces a CSINC whose condition field is `NV` / `AL`: a reserved
encoding. Reference assemblers reject these:

```
$ echo 'cset x0, al' | llvm-mc-18 -triple=aarch64 -show-encoding
<stdin>:1:12: error: condition codes AL and NV are invalid for this instruction
$ echo 'cset x0, nv' | llvm-mc-18 -triple=aarch64 -show-encoding
<stdin>:1:12: error: condition codes AL and NV are invalid for this instruction
```

`encode_cset` performs **no** such check and happily emits the reserved word.

The same bug affects the sibling `encode_csetm` (alias of `CSINV`, same AL/NV rule).

## Reproducer

Minimal failing input found by `prop_rejects_al_nv_conditions`:

```
case = 0      # i.e. cset x0, al
```

```rust
let ops = vec![Operand::Reg("x0".into()), Operand::Cond("al".into())];
assert_eq!(
    encode_cset(&ops),
    Ok(EncodeResult::Word(0x9A9FF7E0))   // silently encoded!
);
// 0x9A9FF7E0 == sf(1) op(0) S(0) 11010100 Rm(=11111) inv_cond(=1111=NV)
//               o2(0) o1(1) Rn(=11111) Rd(=0)
//                              ^^^^^^^^^ cond field == 1111 (NV) — RESERVED.
```

The expected behaviour is `Err(..)`. `cset x0, nv` is likewise accepted (cond field
becomes `1110` = `AL`, also reserved).

## Root cause

`encode_cset` computes `inv_cond = cond ^ 1` and places it in `[15:12]` without
ever testing `cond` (or `inv_cond`) against the reserved values:

```rust
pub(crate) fn encode_cset(operands: &[Operand]) -> Result<EncodeResult, String> {
    ...
    let inv_cond = cond ^ 1; // invert least significant bit
    let word = (sf << 31) | (0b11010100 << 21)
        | (0b11111 << 16) | (inv_cond << 12) | (0b01 << 10) | (0b11111 << 5) | rd;
    Ok(EncodeResult::Word(word))   // no validation that cond != AL/NV
}
```

## Suggested fix

Reject `AL`/`NV` after decoding the condition (applies to `encode_csetm` too):

```rust
let cond = match operands.get(1) {
    Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
    _ => return Err("cset requires condition".to_string()),
};
// CSET/CSETM are aliases of CSINC/CSINV; the aliased condition must not be
// AL (14) or NV (15) — invert swaps the two, so reject both up front.
if matches!(cond, 14 | 15) {
    return Err(format!("cset: condition code AL/NV is invalid for this instruction"));
}
```

## Test status

`prop_encode_cset_tests` (6 properties):

- `prop_opcode_structure_and_fields` .... PASS
- `prop_cset_equals_csinc_xzr_alias` ... PASS  (differential vs `encode_csinc`)
- `prop_cond_inverted_and_aliases` ..... PASS
- `prop_rejects_invalid_operands` ..... PASS
- `prop_rejects_fp_simd_registers` .... **FAIL** (shared family gap — see
  `encode_csel_silently_accepts_fp_simd_registers.md`, which already lists
  `encode_cset` as affected; CSET is GP-only per ARM ARM C6.2.43)
- `prop_rejects_al_nv_conditions` ..... **FAIL** (this bug, CSET-specific)

Happy-path correctness was cross-checked against `llvm-mc-18`:

```
cset x0, eq   -> 0x9A9F17E0  (matches llvm-mc [0xe0,0x17,0x9f,0x9a])
cset w5, ne   -> 0x1A9F07E5  (matches [0xe5,0x07,0x9f,0x1a])
cset x10, gt  -> 0x9A9FD7EA  (matches [0xea,0xd7,0x9f,0x9a])
cset w31, cs  -> 0x1A9F37FF  (matches [0xff,0x37,0x9f,0x1a]; llvm normalizes w31->wzr, cs->hs)
```
