# Bug Report: `encode_ldtr_sized` silently truncates out-of-range 9-bit offsets

**Location:** `src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldtr_sized` (lines 291–314)

## Summary

`encode_ldtr_sized` casts the source offset to `i32` and masks it with `& 0x1FF`
without checking that it lies in the signed 9-bit range `[-256, +255]` of the
LDTR/STTR `imm9` field. An out-of-range offset is therefore silently folded into
the field modulo 512 and returned as `Ok`, producing an instruction whose
effective address differs from the source text.

Per the ARMv8 ARM the LDTR/STTR encoding is `size 111 V 0 0 opc 0 imm9 10 Rn Rt`,
where `imm9` (bits `[20:12]`) is *"the signed immediate byte offset, in the range
-256 to 255."* Values outside that range are unrepresentable and must be rejected.

## Minimal counterexample (witness)

Shrunk by proptest from the failing negative-contract property
`prop_out_of_range_offset_rejected`:

- **Input:** `encode_ldtr_sized(&[Operand::Reg("x0"), Operand::Mem{base:"x1", offset:256}], false, 0)`
- **Expected:** `Err` (256 > 255, out of signed-9-bit range)
- **Actual:** `Ok(EncodeResult::Word(940574752))`

Decoding the emitted word's `imm9` field (`bits [20:12] = 0x100`) sign-extends to
**-256** — i.e. `ldtr x0, [x1, #256]` assembles as if it were
`ldtr x0, [x1, #-256]`. The instruction dereferences the wrong address with no
diagnostic (miscompilation).

**Reproduce:**

```
cargo test --lib prop_encode_ldtr_sized_offset_tests
```

Two properties fail with shrunk witnesses:

| Property | Shrunk minimal input | Actual |
|---|---|---|
| `prop_out_of_range_offset_rejected` | `is_load=false, size=0, off=256` | `Ok(Word(940574752))` |
| `prop_common_offsets_rejected` | `is_load=false, off_idx=0` (`off=256`) | `Ok(Word(4161800224))` |

## Root cause

```rust
let (rn, imm9) = match &operands[1] {
    Operand::Mem { base, offset } => {
        let rn = parse_reg_num(base).ok_or("invalid base reg")?;
        (rn, *offset as i32)            // i64 -> i32, no range check   (line 307)
    }
    _ => return Err("ldtr/sttr: expected memory operand".to_string()),
};
let imm9_enc = (imm9 as u32) & 0x1FF;   // silent 9-bit truncation       (line 311)
```

Line 307 widens `i64`→`i32` with no range guard; line 311 keeps only the low 9
bits. Any offset outside `[-256, 255]` is folded into the field modulo 512.

## Additional manifestations (same single bug)

The same unchecked mask folds every out-of-range offset; e.g. `+512`→decoded `0`,
`+1000`→decoded `-24`, `-257`→decoded `-1`, `-512`→decoded `0`. These are all one
root cause (missing range validation), not separate findings.

## Impact

- Miscompilation: the assembled object does not match the source; the load/store
  hits the wrong address.
- Silent: no error or warning is emitted, so the defect reaches linked binaries.

## Suggested fix

Validate the offset before masking:

```rust
let imm9 = *offset as i32;
if !(-256..=255).contains(&imm9) {
    return Err(format!("ldtr/sttr offset {} out of range [-256, 255]", imm9));
}
let imm9_enc = (imm9 as u32) & 0x1FF;
```

This satisfies the ARM ARM `imm9` range and turns both negative-contract
properties into passing tests.
