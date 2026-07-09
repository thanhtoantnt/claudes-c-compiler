# Bug — `encode_neon_tbx` panics on an empty register list

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_tbx`
**Severity:** medium (robustness / abort instead of diagnostic)
**Witness test:** `tbx_rejects_empty_register_list`
(`#[ignore]`d in `src/backend/arm/assembler/encoder/neon_tbx_pbt.rs`)

## Defect

```rust
let (rn, num_regs) = match &operands[1] {
    Operand::RegList(regs) => {
        let first_reg = match &regs[0] {        // indexes element 0 unconditionally
            ...
        };
        (first_reg, regs.len() as u32)
    }
    ...
};
```

When the table operand is an empty `Operand::RegList(vec![])`, the encoder
indexes `regs[0]` and **panics** (index out of bounds) instead of returning
`Err`. A malformed AST should never crash the assembler.

## Minimal failing input

operands = `[ va(0,"16b"), Operand::RegList(vec![]), va(2,"16b") ]`

## Expected vs. actual

- **Expected:** `Err` (empty table register list is not a legal instruction).
- **Actual:** `panic` ("index out of bounds: the len is 0 but the index is 0").

## Impact

Any path that hands `encode_neon_tbx` an empty register list (a parser bug, a
macro expansion edge case, or hand-built IR) crashes the whole assembler
rather than emitting a recoverable error.

## Fix

Guard before indexing:

```rust
Operand::RegList(regs) => {
    if regs.is_empty() {
        return Err("tbx: table register list is empty".to_string());
    }
    let first_reg = match &regs[0] { ... };
    (first_reg, regs.len() as u32)
}
```
(or use `regs.first()`).

## Reproduce

```bash
cargo test --lib neon_tbx_pbt -- --ignored tbx_rejects_empty_register_list
```
