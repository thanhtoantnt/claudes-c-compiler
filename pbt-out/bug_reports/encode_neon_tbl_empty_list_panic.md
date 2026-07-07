# BUG: `encode_neon_tbl` panics on an empty table register list

- **File:** `src/backend/arm/assembler/encoder/neon.rs`
- **Function:** `encode_neon_tbl`
- **Severity:** Medium (abort/crash instead of graceful error; reachable via `pub(crate)`)
- **Found by:** property `tbl_pbt_tests::prop_empty_list_does_not_panic`

## Summary

`encode_neon_tbl` indexes the table-register vector unconditionally with `&regs[0]`
before checking whether the list is non-empty. When the second operand is an
**empty** `Operand::RegList(vec![])`, this panics with an index-out-of-bounds
instead of returning `Err`, violating the error contract every other malformed-input
path in the function honors.

## Reproduction

```rust
use ccc::backend::arm::assembler::encoder::neon::*; // via crate-internal test
// TBL Vd.8b, {}, Vm.8b  — empty table register list
let ops = vec![
    Operand::RegArrangement { reg: "v0".into(), arrangement: "8b".into() },
    Operand::RegList(vec![]),
    Operand::RegArrangement { reg: "v0".into(), arrangement: "8b".into() },
];
encode_neon_tbl(&ops); // panics at neon.rs:781
```

```
thread '...' panicked at src/backend/arm/assembler/encoder/neon.rs:781:40:
index out of bounds: the len is 0 but the index is 0
```

## Root cause

```rust
let (rn, num_regs) = match &operands[1] {
    Operand::RegList(regs) => {
        let first_reg = match &regs[0] {            // <-- BUG: no emptiness check
            Operand::RegArrangement { reg, .. } => parse_reg_num(reg).ok_or("invalid reg")?,
            Operand::Reg(name) => parse_reg_num(name).ok_or("invalid reg")?,
            _ => return Err("tbl: expected register in list".to_string()),
        };
        (first_reg, regs.len() as u32)
    }
    _ => return Err("tbl: expected register list as second operand".to_string()),
};
```

`regs` is never guarded with `is_empty()`. The same latent pattern exists in the
sibling `encode_neon_tbx` (immediately below in the same file), which also does
`match &regs[0]`.

## Suggested fix

Guard the empty list (and, optionally, the ISA's 1–4 register limit, which the
encoder currently does **not** enforce — it silently truncates `len = (n-1) & 0x3`
so a 5-register table produces `len=0`, indistinguishable from a 1-register table):

```rust
Operand::RegList(regs) => {
    if regs.is_empty() {
        return Err("tbl: table register list must be non-empty".to_string());
    }
    // (optional) if regs.len() > 4 { return Err(...); }
    let first_reg = match &regs[0] { /* ... unchanged ... */ };
    (first_reg, regs.len() as u32)
}
```

## Test coverage

The new `tbl_pbt_tests` module (same file) adds 7 properties:

| # | Property | Result |
|---|----------|--------|
| 1 | `prop_fixed_fields` — constant ISA bits (31, 29-24, 23-21, 15, 12, 11-10) | ✅ pass |
| 2 | `prop_matches_reference_encoding` — differential oracle vs independent layout | ✅ pass |
| 3 | `prop_register_fields_preserved` — Rd/Rn/Rm round-trip | ✅ pass |
| 4 | `prop_len_field` — `len` = (num_regs−1) for 1–4 regs | ✅ pass |
| 5 | `prop_q_bit` — Q=1 iff `.16b` | ✅ pass |
| 6 | `prop_error_contracts` — <3 ops / non-RegList / bad reg → Err | ✅ pass |
| 7 | `prop_empty_list_does_not_panic` — empty list must Err, not panic | ❌ **FAIL (this bug)** |

Note: the empty-list case is currently a panic; `prop_empty_list_does_not_panic`
catches it via `catch_unwind` so the failure is reported cleanly. The regression
seed is recorded in `proptest-regressions/backend/arm/assembler/encoder/neon.txt`.
