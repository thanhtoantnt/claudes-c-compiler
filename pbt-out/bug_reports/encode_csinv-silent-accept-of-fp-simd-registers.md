# BUG: `encode_csinv` (and the whole conditional-select family) silently accepts FP/SIMD register operands

## Severity
High — produces a well-formed but semantically wrong instruction with no diagnostic.

## Status
Confirmed by property-based testing (`prop_encode_csinv_tests::prop_rejects_fp_simd_registers`,
`src/backend/arm/assembler/encoder/compare_branch.rs`). The pre-existing CSINC suite
already fails identically on this (`prop_encode_csinc_tests::prop_rejects_invalid_operands`,
case 10), so the defect is shared across the family.

## Summary
`encode_csinv` encodes `CSINV` — an instruction defined **only on general-purpose
(X/W) registers** (ARM ARM C4.1.67, "Conditional Select (invert)"). When given a
floating-point / SIMD register name (`d0`, `s1`, `q2`, `v3`, `h4`, `b5`) in the
Rd / Rn / Rm slot, it does **not** reject it. Instead it silently re-encodes the
register's numeric index as if it were a GP register, emitting a valid-looking
`CSINV` word with the wrong register operand.

Root cause is `parse_reg_num` (`src/backend/arm/assembler/encoder/mod.rs:131`),
which matches every FP/SIMD prefix:

```rust
match prefix {
    'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // <-- accepts FP/SIMD
        let num: u32 = name[1..].parse().ok()?;
        if num <= 31 { Some(num) } else { None }
    }
    _ => None,
}
```

`get_reg` (mod.rs:956) calls `parse_reg_num` and therefore never rejects FP/SIMD
names; `encode_csinv` / `encode_csel` / `encode_csinc` / `encode_csneg` /
`encode_cinv` / `encode_cinc` / `encode_cneg` all share this path.

## Reproduction
```
cargo test --lib prop_encode_csinv_tests
```
Minimal failing input reported by proptest:
```
prefix = "b", n = 0, slot = 0      →  encode_csinv(&[b0, x1, x2, eq]) == Ok(Word(0x5A82_0020))
```
`b0` is encoded as GP register `Rd=0` (sf taken from the FP name → 32-bit), so a
SIMD byte-register operand becomes a 32-bit `CSINV w0, w1, w2, eq` with no error.

## Expected behaviour
Any FP/SIMD register name in the Rd/Rn/Rm slots of a conditional-select
instruction must be rejected (`Err`), per the ARM ARM which lists these
instructions as GP-register-only. There is no spec-authorised wrapping here.

## Suggested fix
Validate the register class inside `get_reg` (or a dedicated GP-register helper),
e.g. reject when `is_fp_reg(name)` (mod.rs already defines `is_fp_reg` at line
~160 but `get_reg` does not call it). This single fix closes the gap for the
entire conditional-select family (and any other GP-only consumer of `get_reg`).

## Test suite
`prop_encode_csinv_tests` in `src/backend/arm/assembler/encoder/compare_branch.rs`:

| Property | Result | Oracle |
|---|---|---|
| `prop_opcode_structure_and_fields` | PASS | structural / field-placement |
| `prop_csinv_xor_csel_is_bit30` | PASS | differential (CSINV vs CSEL differ only in bit 30) |
| `prop_sf_bit_is_bit31` | PASS | differential (64- vs 32-bit Rd differ only in sf) |
| `prop_cond_round_trips_and_aliases` | PASS | condition-code mapping + cs/hs, cc/lo aliases |
| `prop_rejects_invalid_operands` | PASS | negative contract (arity / wrong operand kind / missing cond) |
| `prop_rejects_fp_simd_registers` | **FAIL** | negative contract (register class) — **this bug** |
