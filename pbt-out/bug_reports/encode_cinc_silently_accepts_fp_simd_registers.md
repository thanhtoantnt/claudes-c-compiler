# Bug Report: `encode_cinc` silently accepts FP/SIMD registers (re-encodes them as GP)

**Location:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_cinc`

## Summary

`encode_cinc` resolves its register operands (`Rd`, `Rn`) through the shared
`get_reg` helper, which in turn calls `parse_reg_num`. `parse_reg_num` happily
accepts the floating-point/SIMD register-name prefixes `d`, `s`, `q`, `v`, `h`,
`b` and returns their numeric index as if they were general-purpose registers.
`is_64bit_reg` then reports `false` for any of them, so the operand is
re-encoded as a 32-bit W register.

`CINC` (ARM ARM C6.2.45) is defined **only** on general-purpose (X/W) registers.
As a result, `encode_cinc` accepts FP/SIMD operand names and silently emits a
CSINC whose register fields hold the FP register's numeric index, producing a
**different instruction** with no diagnostic.

This is the same root cause already characterized for the sibling conditional-
select encoders in this same file (`encode_csel`, `encode_csinc`, `encode_csinv`,
`encode_csneg`, `encode_cset`, `encode_csetm`, `encode_cneg`, `encode_cinv`),
but `encode_cinc` itself is affected independently and needs its own fix.

## Reproduction

The PBT property `prop_encode_cinc_tests::prop_rejects_invalid_operands`
**fails**. Minimal failing input (proptest-shrunk):

```
case = 10   -> encode_cinc([Reg("d0"), Reg("x0"), Cond("eq")])
```

```
encode_cinc should reject case 10 (got Ok(Word(444601344)))
```

Root cause (shared helper):

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    let name = name.to_lowercase();
    ...
        'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // <-- FP/SIMD prefixes accepted
            let num: u32 = name[1..].parse().ok()?;
            if num <= 31 { Some(num) } else { None }
        }
    ...
}
```

```rust
pub(crate) fn encode_cinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;   // no register-class check
    let (rn, _) = get_reg(operands, 1)?;       // no register-class check
    ...
}
```

Decoding the emitted word:

```
encode_cinc([d0, x0, eq]) -> Ok(Word(0x1A801400))
0x1A801400 == csinc w0, w0, w0, ne
```

The FP source register `d0` (double-precision V0) was silently reinterpreted as
`w0` (register index 0, 32-bit because `is_64bit_reg("d0") == false`). The FP
register identity is lost.

## Impact

Silent miscompilation with a register-class error. An assembler that should
reject `cinc d0, x0, eq` (CINC is GP-only) instead emits
`csinc w0, w0, w0, ne`, a semantically and architecturally distinct instruction.
Hand-written or generated assembly that uses the wrong register class for a
conditional-increment will assemble "successfully" and silently produce wrong
code rather than failing fast.

## Suggested fix

Reject non-GP register names in `encode_cinc` before calling `get_reg`
(consistent with the fix needed for the other conditional-select encoders):

```rust
fn ensure_gp_reg(operands: &[Operand], idx: usize) -> Result<(), String> {
    if let Some(Operand::Reg(name)) = operands.get(idx) {
        let lo = name.to_lowercase();
        let is_gp = lo.starts_with('x') || lo.starts_with('w')
            || matches!(lo.as_str(), "sp" | "wsp" | "xzr" | "wzr" | "lr");
        if !is_gp {
            return Err(format!(
                "cinc: operand {} ({}) must be a general-purpose register",
                idx, name
            ));
        }
    }
    Ok(())
}

pub(crate) fn encode_cinc(operands: &[Operand]) -> Result<EncodeResult, String> {
    ensure_gp_reg(operands, 0)?;
    ensure_gp_reg(operands, 1)?;
    let (rd, is_64) = get_reg(operands, 0)?;
    ...
}
```

## Test coverage added

Five `proptest!` properties added to `prop_encode_cinc_tests` in
`compare_branch.rs`:

| Property | Oracle | Result |
|---|---|---|
| `prop_opcode_structure_and_fields` | Reference (ARM ARM C4.1.66 CSINC field layout, alias Rm=Rn) | PASS |
| `prop_cinc_equals_csinc_alias` | Differential vs `encode_csinc` (`CINC Rd,Rn,c` == `CSINC Rd,Rn,Rn,!c`) | PASS |
| `prop_rm_equals_rn` | Invariant (alias sets Rm field == Rn field) | PASS |
| `prop_condition_is_inverted` | Algebraic (cond field == `encode_cond(c)^1`; complementary pairs differ only in bit 12) | PASS |
| `prop_rejects_invalid_operands` | Negative/error contract (this finding, cases 10-15) | **FAIL** |

## Regression property

Failing property: `prop_rejects_invalid_operands`

```rust
prop_assert!(encode_cinc(&[xreg(rd), xreg(rn), xreg(rm)], "eq").is_err());
```
