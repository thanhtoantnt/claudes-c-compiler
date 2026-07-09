# Bug Report — `encode_csel` silently accepts mismatched register widths

- **Function:** `encode_csel`
- **File:** `src/backend/arm/assembler/encoder/compare_branch.rs`
- **Instruction class:** CSEL (Conditional Select), ARM ARM C4.1.64 / C5.6.21
- **Severity:** Medium (silent mis-encoding; emits semantically wrong object code instead of an assembler error)
- **Category:** Missing consistency validation (negative-contract violation)

## Summary

`encode_csel` derives the `sf` (register-width) bit **solely from the destination register `Rd`** and discards the width of `Rn` and `Rm`. An operand set that mixes 64-bit (X) and 32-bit (W) registers — which the architecture requires to be uniform — is silently accepted and emitted as either a 64-bit or 32-bit instruction depending only on `Rd`, rather than being rejected with an error.

## Spec basis (cited)

- **ARM ARM, CSEL (C5.6.21):** the `sf` field applies to the *whole* instruction. The encoding is only valid when `Rd`, `Rn`, and `Rm` are all the same width — all X (sf=1) or all W (sf=0).
- **GAS / LLVM-MC behavior:** both assemblers reject a width-mismatched CSEL, e.g. `csel x0, w1, x2, eq` → `Error: operand size mismatch`. No cited spec permits the encoder to coerce the operands to `Rd`'s width.

## Root cause

In `src/backend/arm/assembler/encoder/compare_branch.rs`:

```rust
pub(crate) fn encode_csel(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, _) = get_reg(operands, 1)?;   // ← width discarded (bound to `_`)
    let (rm, _) = get_reg(operands, 2)?;   // ← width discarded (bound to `_`)
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csel requires condition".to_string()),
    };
    let sf = sf_bit(is_64);                 // ← derived ONLY from Rd
    let word = ((sf << 31) | (0b11010100 << 21)
        | (rm << 16) | (cond << 12)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

`get_reg` returns `(num, is_64)`, but the `is_64` values for `Rn` and `Rm` are bound to `_` and never compared against the width of `Rd`. Therefore a width mismatch is invisible to `encode_csel`.

## Minimal reproduction

**Input:** `csel x0, w1, w2, eq`

**Actual:** `encode_csel` returns `Ok(Word(0x1A82_0020))`, a 64-bit CSEL:

```
0x1A82_0020  =  sf=1 0 0 11010100 Rm=00010 cond=0000 00 Rn=00001 Rd=00000
            →  CSEL X0, X1, X2, EQ   (note: X1/X2, not the W1/W2 the user wrote)
```

The operand *names* `w1`/`w2` are silently rewritten to `x1`/`x2`.

**Expected:** `encode_csel` returns `Err` because `Rn`/`Rm` are 32-bit while `Rd` is 64-bit.

## Impact

- **Correctness:** downstream consumers (codegen, assembler) receive an instruction the user never wrote, with no signal that the operands were coerced. A 32-bit W register used as a 64-bit source reads undefined high bits; a 64-bit X register truncated to W loses data. Neither matches the source's intent.
- **Assembler-contract break:** every real AArch64 assembler treats this as a hard error; silently accepting it makes this encoder unable to catch typos or codegen bugs at the source.

## Suggested fix

Validate width consistency before encoding:

```rust
pub(crate) fn encode_csel(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, is_64) = get_reg(operands, 0)?;
    let (rn, rn_is_64) = get_reg(operands, 1)?;
    let (rm, rm_is_64) = get_reg(operands, 2)?;
    if rn_is_64 != is_64 || rm_is_64 != is_64 {
        return Err("csel: Rd, Rn and Rm must all be the same register width".to_string());
    }
    let cond = match operands.get(3) {
        Some(Operand::Cond(c)) => encode_cond(c).ok_or("invalid cond")?,
        _ => return Err("csel requires condition".to_string()),
    };
    let sf = sf_bit(is_64);
    let word = ((sf << 31) | (0b11010100 << 21) | (rm << 16) | (cond << 12)) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

## Test evidence

The following proptest property (to be added to the existing `prop_encode_csel_tests` module in the same file) fails against the current implementation and passes after the fix. It generates all width combinations, skips the all-same-width (valid) case, and asserts rejection:

```rust
#[test]
fn prop_rejects_mismatched_register_widths(
    rd_is64 in any::<bool>(),
    rn_is64 in any::<bool>(),
    rm_is64 in any::<bool>(),
    n in 0u32..=30u32,
    cond_idx in 0usize..COND_TABLE.len(),
) {
    // Skip the trivial all-same-width case (it is a valid CSEL).
    prop_assume!(!(rd_is64 == rn_is64 && rn_is64 == rm_is64));
    let (cond_name, _) = COND_TABLE[cond_idx];
    let mk = |is64: bool| if is64 { format!("x{}", n) } else { format!("w{}", n) };
    let ops = vec![
        Operand::Reg(mk(rd_is64)),
        Operand::Reg(mk(rn_is64)),
        Operand::Reg(mk(rm_is64)),
        Operand::Cond(cond_name.to_string()),
    ];
    prop_assert!(
        encode_csel(&ops).is_err(),
        "mixed-width csel ({:?}) must be rejected per ARM ARM (Rd/Rn/Rm \
         must share width), got {:?}",
        ops, encode_csel(&ops)
    );
}
```
