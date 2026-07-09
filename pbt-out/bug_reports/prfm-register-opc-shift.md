# Bug: `encode_prfm` register-offset form places `opc` one bit too high

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_prfm`, `MemRegOffset` arm

## Summary

The PRFM **register-offset** form emits the `opc=10` field at **bits [24:23]**
(`0b10 << 23`) instead of **bits [23:22]** (`0b10 << 22`), per ARM ARM §C4.1.90
(PRFM register). This contradicts the function's own comment, the PRFM
**immediate** form's base constant `0xF9800000` (which correctly places `opc=10`
at bits [23:22]), and the sibling `encode_ldrsw` register form (which correctly
uses `0b10 << 22`). The off-by-one shift produces a malformed/unallocated encoding.

## Root cause

```rust
Operand::MemRegOffset { base, index, extend, shift } => {
    ...
    // BUG: should be (0b10 << 22); opc belongs at bits [23:22]
    let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 23) | (1 << 21)
        | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
    Ok(EncodeResult::Word(word))
}
```

## Observed consequences

For `prfm pldl1keep, [x0, x1]` (no extend specifier → default `LSL`, `S=0`), the
encoder produces `0xF9206800` whereas the ARM reference is `0xF8A06800`
(`XOR = 0x01800000`, i.e. exactly bits 24 and 23 swapped). An assembler/
disassembler round-trip would yield an unallocated/incorrect instruction.

The PRFM immediate form and the `prfop` name→value table are correct (verified by
the passing properties `prop_prfm_immediate_matches_reference`,
`prop_prfop_table_and_structure`, and `prfm_error_contract_rejects_invalid_operands`).

## Evidence

```
$ cargo test --lib prop_encode_prfm_tests
test ... prop_prfm_register_offset_matches_reference ...... FAILED

minimal failing input: pidx=0, rn=0, rm=0, opt_idx=0, shift_amt=0
  left (code)  = 0xF9206800
  right (ref)  = 0xF8A06800   (ARM ARM §C4.1.90; anchored to imm-form base 0xF9800000)
```

The immediate-form base `0xF9800000` independently re-derives from the ARM ARM
layout `11 111 0 01 10 imm12 Rn Rt`, which fixes `opc=10` unambiguously at
bits [23:22] — the register form must match.

## Suggested fix

```rust
let word = (0b11 << 30) | (0b111 << 27) | (0b10 << 22) | (1 << 21)
    | (rm << 16) | (option << 13) | (s_bit << 12) | (0b10 << 10) | (rn << 5) | prfop;
```

After this fix, `prop_prfm_register_offset_matches_reference` passes; the other
four PRFM properties are unaffected.
## Regression Property

Failing property: `prop_prfm_register_offset_matches_reference`

```rust
prop_assert_eq!(encode_prfm_reg_offset(&[Operand::Reg("x0".into()), mem_reg_offset(xreg(1), xreg(2), "lsl", 0)]),
               Ok(EncodeResult::Word(0xF8A06800)));  // ARM ARM reference
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/134
