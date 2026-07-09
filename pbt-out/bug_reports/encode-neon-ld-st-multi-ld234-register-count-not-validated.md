# Bug: `encode_neon_ld_st_multi` ignores register-list length for LD2/3/4

| Field | Value |
|---|---|
| Function | `encode_neon_ld_st_multi` |
| File | `src/backend/arm/assembler/encoder/neon.rs` (function `encode_neon_ld_st_multi`) |
| Severity | Correctness / silent mis-assembly |
| Discovered by | property `rejects_register_count_mismatch` in `neon_ld_st_multi_pbt.rs` |
| Status | **Confirmed** (witness fails when un-`#[ignore]`d; 8 sibling properties + golden table pass) |

## Summary

For `num_structs >= 2` the opcode is selected without consulting `num_regs`:

```rust
let opcode = match num_structs {
    1 => match num_regs { 1 => 0b0111, 2 => 0b1010, 3 => 0b0110, 4 => 0b0010, _ => return Err(..) },
    2 => 0b1000u32,   // <-- num_regs ignored
    3 => 0b0100,
    4 => 0b0000,
    _ => return Err(..),
};
```

The ARM ARM requires the register-list length to equal the structure count for
LD2/ST2/LD3/ST3/LD4/ST4; `llvm-mc-18` rejects mismatches with
`error: invalid operand for instruction`. (Note: LD1 *is* validated by its
inner `num_regs` match-arm — only LD2/3/4 are affected.)

## Minimal input

`ld2 {v0.8b}, [x1]`  (a 1-register list for a 2-structure instruction)

## Expected vs. actual

- Expected: `Err`.
- Actual: `Ok(EncodeResult::Word(0x0C408020))` — a valid-looking LD2 word.

`ld2 {v0.8b, v1.8b, v2.8b}, [x1]` (3 registers) is accepted the same way.

## Impact

Malformed operands produce a plausible 32-bit word instead of `Result::Err`, so
the defect silently propagates to emitted machine code rather than failing at
assemble time.

## Suggested fix

For `num_structs in 2..=4`, require `num_regs == num_structs` (analogous to the
LD1 inner match-arm). Additionally, check register consecutiveness, which is
currently a `TODO` in the sibling `encode_neon_ld_st_single`.

## Verification

`cargo test --lib neon_ld_st_multi` → 8 passed, 2 ignored (default suite green).
`cargo test --lib neon_ld_st_multi -- --ignored` → this witness fails with the
shrunk input above. Absolute correctness is pinned by an 18-case golden table
captured from `llvm-mc-18`.
