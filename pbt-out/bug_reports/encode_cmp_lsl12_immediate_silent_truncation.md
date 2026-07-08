# Bug Report — `encode_cmp` silently truncates an oversized `lsl #12` immediate

**Entry point under test:** `encode_cmp` in
`src/backend/arm/assembler/encoder/compare_branch.rs`

**Root cause (where the bug lives):** `encode_add_sub` (immediate path) in
`src/backend/arm/assembler/encoder/data_processing.rs`

**Severity:** Medium (assembles an instruction the programmer did not write,
with no diagnostic; downstream consumers see a silently-corrupted immediate)

## Summary

`encode_cmp` is a thin wrapper: it rewrites `CMP Rn, op` →
`SUBS ZR, Rn, op` (prepends `XZR`/`WZR`, sets `is_sub = true`,
`set_flags = true`) and forwards all operands — including a trailing
`Shift { lsl, 12 }` — verbatim to `encode_add_sub`.

In `encode_add_sub`'s immediate path, the *explicit* `lsl #12` form takes the
immediate as-is and masks it into the 12-bit `imm12` field:

```rust
let explicit_shift = if operands.len() > 3 {
    if let Some(Operand::Shift { kind, amount }) = operands.get(3) {
        kind == "lsl" && *amount == 12
    } else { false }
} else { false };

let (imm12, sh) = if explicit_shift {
    // Explicit lsl #12: use the immediate as-is (must fit in 12 bits)
    ((imm_val as u32) & 0xFFF, 1u32)        // <-- masks instead of validating
} else if imm_val <= 0xFFF {
    ...
```

The comment admits the immediate "must fit in 12 bits", but the code masks with
`& 0xFFF` rather than checking the range, so an out-of-range value is silently
truncated instead of rejected.

Per the ARMv8 ARM (add/subtract immediate, C4.1.4), `imm12` is a 12-bit
*unsigned* field: the shifted (`lsl #12`) variant still requires the immediate
to be in `[0, 4095]`. An immediate above `0xFFF` is not encodable. GAS and
LLVM-MC reject it:

```
$ echo "cmp x0, #0x1001, lsl #12" | llvm-mc -triple=aarch64 -
<instantiation>:1:1: error: expected compatible register or logical immediate
```

## Reproduction

Property `prop_rejects_large_imm_with_explicit_shift` (negative contract) in
`prop_encode_cmp_tests` fails with minimized input:

```
rn_name = "w0", bad_imm = 4097, shift = lsl #12
```

The call returns

```
Ok(EncodeResult::Word(1900020767))   // == 0x7140_083F
```

Decoded: `sf=0 op=1 S=1 10001 sh=1 imm12=1 Rn=0 Rd=31` — i.e. it assembles
`cmp w0, #4097, lsl #12` as if the user had written `cmp w0, #1, lsl #12`,
because `4097 (0x1001) & 0xFFF == 1`.

The plain (unshifted) immediate path is correct: `prop_rejects_unencodable_immediate`
confirms `cmp w0, #4097` (no shift) returns `Err`, as it should. The bug is
isolated to the explicit-`lsl #12` branch.

## Expected behavior

`CMP Rn, #imm, lsl #12` with `imm > 0xFFF` (and `imm < 0` after the negative
flip) must return `Err`, matching the unshifted path and GAS/LLVM-MC.

## Suggested fix

In `encode_add_sub`, validate the immediate before masking in the explicit-shift
branch:

```rust
let (imm12, sh) = if explicit_shift {
    if imm_val > 0xFFF {
        return Err(format!(
            "immediate {} does not fit in 12-bit imm12 field for lsl #12 form", imm_val));
    }
    (imm_val as u32, 1u32)
} else ...
```

(Range validation belongs in `encode_add_sub`; `encode_cmp` correctly forwards
the operands and will then propagate the `Err` automatically.)

## What passes (verified by the new suite)

All other `encode_cmp` behaviour is correct for the tested inputs:

- Immediate form (`CMP Rn, #imm`, `imm ∈ [0, 0xFFF]`): full structural oracle —
  `sf`, `op=1`, `S=1`, opcode `10001`, `sh=0`, `imm12` round-trips, `Rn`,
  `Rd=31`.
- Register form (`CMP Rn, Rm`): full structural oracle — opcode `01011`,
  `shift=0`, `Rm`, `imm6=0`, `Rn`, `Rd=31`.
- Width differential: `CMP Xn, op` vs `CMP Wn, op` differ only in bit 31.
- Cross-instruction differential: `CMP` vs `CMN` (sibling) differ only in
  bit 30 (op = sub vs add).
- Plain unshifted immediate range validation: out-of-range `imm` (e.g. `#0x1001`)
  is correctly rejected.

## Regression property

Failing property: `prop_rejects_large_imm_with_explicit_shift`

```rust
prop_assert!(encode_cmp(&[wreg(rn), Operand::Imm(4097), Operand::Shift { kind: "lsl".into(), amount: 12 }]).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/26
