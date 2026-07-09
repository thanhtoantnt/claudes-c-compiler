# Bug Report: `encode_neon_two` silently accepts invalid mnemonic

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_two`
**Severity:** High

## Summary

`encode_neon_two` is the fallback handler for any NEON mnemonic not matched to specific encoders. It accepts unknown mnemonics without validation, treating them as 2-element NEON instructions, which could be garbage or typos. The function produces garbage encodings without any diagnostic.

## Root Cause

```rust
pub(crate) fn encode_neon_two(operands: &[Operand]) -> Result<EncodeResult, string> {
    // Fallback for NEON instructions not yet covered by specific encoders
    let (rd, _) = get_neon_reg(operands, 0)?;
    let (rn, _) = get_neon_reg(operands, 1)?;
    let word = (0 << 30) | (0b110 << 27) | (0b10 << 22) | (rm << 16)
             | (0b10 << 10) | (rn << 5) | rd;
    Ok(EncodeResult::Word(word))
}
```

The function derives encoding bits from arrangement string but never validates the mnemonic.

## Reproduction

**Input:** `foo v0.8b, v1.8b`

**Expected:** `Err` — unknown NEON mnemonic: foo

**Actual:** `Ok(Word(0x7A400000))` — garbage encoding

**Minimal failing input:** any unknown mnemonic

## Impact

Unknown mnemonics accepted as NEON TWO (2-register) instructions, producing garbage encodings without diagnostic. Typos or parser bugs silently corrupt output.

## Suggested Fix

Validate mnemonic against list of valid NEON TWO mnemonics or remove the fallback entirely:

```rust
// Option 1: Reject unknown mnemonics in the fallback
let valid_two = ["pmull", "pmull", "umlal", "umlal"];
if !valid_two.contains(&mnemonic) {
    return Err(format!("unknown NEON mnemonic: {}", mnemonic));
}

// Option 2: Remove the fallback handler entirely
pub(crate) fn encode_neon_two(operands: &[Operand]) -> Result<EncodeResult, string> {
    Err("encode_neon_two not yet implemented".into())
}
```

## Regression Property

Failing property: `neon_two_rejects_unknown_mnemonics`

```rust
prop_assert!(encode_neon_two(&[neon_reg(0, "8b"), neon_reg(1, "8b")]).is_err());
prop_assert!(encode_neon_two(&[neon_reg(0, "8b"), neon_reg(1, "8b")]).is_err());  // 2-register form
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/182