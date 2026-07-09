# Bug: `encode_neon_mls` accepts unallocated doubleword arrangements (`.1d` / `.2d`)

## Status
Functional finding — documented by the `#[ignore]`d regression test
`neon_mls_pbt::mls_rejects_doubleword` (reproduces the failure).

## Target
`src/backend/arm/assembler/encoder/neon.rs` — `encode_neon_mls`

```rust
pub(crate) fn encode_neon_mls(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, arr_d) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let (rm, _) = get_neon_reg(operands, 2)?;
    let (q, size) = neon_arr_to_q_size(&arr_d)?;          // <-- accepts size==0b11
    // MLS: 0 Q 1 01110 size 1 Rm 10010 1 Rn Rd (U=1)
    let word = (q << 30) | (1 << 29) | (0b01110 << 24) | (size << 22) | (1 << 21)
        | (rm << 16) | (0b100101 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Root cause
`neon_arr_to_q_size` maps `1d` → `(0, 0b11)` and `2d` → `(1, 0b11)`, i.e.
`size = 0b11`. In the "Advanced SIMD three same" encoding group, the multiply
family (`MUL` / `MLA` / `MLS`) is architecturally defined **only for
`size != 0b11`** (ARMv8-A ARM, "Advanced SIMD three same": the `size == 11`
row is UNALLOCATED for these mnemonics). `encode_neon_mls` makes no check on
`size` and happily emits a 32-bit word with `size = 11`, producing a
silently-wrong / unallocated encoding instead of returning `Err`.

## Evidence

### 1. Reference assembler (`llvm-mc-18`) rejects the doubleword forms
```
$ printf 'mls v3.1d, v4.1d, v5.1d\nmls v3.2d, v4.2d, v5.2d\n' \
  | llvm-mc-18 --triple=aarch64 --assemble --show-encoding
<stdin>:1:5: error: invalid operand for instruction
mls v3.1d, v4.1d, v5.1d
    ^
<stdin>:2:5: error: invalid operand for instruction
mls v3.2d, v4.2d, v5.2d
    ^
```

### 2. This crate's encoder accepts them
Running the ignored regression test:
```
$ cargo test --lib neon_mls_pbt::mls_rejects_doubleword -- --ignored
...
MLS does not support .1d (size=0b11 is unallocated); expected Err but got Ok(0x2EE29420)
test result: FAILED. 0 passed; 1 failed
```
`mls v0.1d, v1.1d, v2.1d` produces `0x2EE29420` (a word with `size=11`,
which is unallocated for this instruction) instead of an error.

## Impact
- **Silent mis-assembly.** A program containing `mls v0.1d, ...` (a typo or
  bad codegen) is accepted and turned into an undefined/unallocated 32-bit
  instruction word rather than a compile-time error.
- **Consistency with the existing `MLA` finding.** The sibling
  `encode_neon_mla` has the identical defect (see `MLA_DWORD_BUG_REPORT.md`);
  `encode_neon_mul` shares the same three-same layout and the same latent gap.
  All three are part of the multiply family and should enforce `size != 0b11`.

## Suggested fix
After computing `(q, size)` from the arrangement, reject the unallocated
doubleword size before emitting:

```rust
let (q, size) = neon_arr_to_q_size(&arr_d)?;
if size == 0b11 {
    return Err(format!(
        "MLS does not support {arr_d} (size=0b11 is unallocated); \
         valid arrangements are .8b/.16b/.4h/.8h/.2s/.4s"
    ));
}
```

(Apply the same guard to `encode_neon_mla` and `encode_neon_mul` for parity.)

## Coverage
- Golden table (6 cases, llvm-mc-18 verified) — **PASS**
- `matches_reference_encoder` (differential) — **PASS**
- `fields_round_trip_and_map_arrangement` — **PASS**
- `fixed_bits_are_constant` (incl. `U==1`) — **PASS**
- `rejects_unsupported_arrangement` (unknown arrangement strings) — **PASS**
- `mls_rejects_doubleword` (the `.1d`/`.2d` contract) — **FAILS, ignored**
