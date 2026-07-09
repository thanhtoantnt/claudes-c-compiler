# Bug Report: `encode_adr` silently truncates out-of-range immediates

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_adr`
**Severity:** High

## Summary

`encode_adr` accepts an arbitrary `i64` immediate and packs it into the ADR 21-bit signed immediate field using bit-masking (`& 3` for `immlo`, `& 0x7FFFF` for `immhi`) **without validating the encodable range**. The code itself flags this with a `TODO: validate 21-bit signed immediate range`.

Immediates outside the legal range `[-2^20, 2^20-1] = [-1048576, 1048575]` are **silently truncated** instead of being rejected, producing an ADR instruction that loads a *different* address than the source requested — with no error.

## Root Cause

```rust
pub(crate) fn encode_adr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rd, _) = get_reg(operands, 0)?;
    // TODO: validate 21-bit signed immediate range
    if let Some(Operand::Imm(imm)) = operands.get(1) {
        let imm = *imm;
        // ADR: 0 immlo[1:0] 10000 immhi[18:0] Rd
        let immlo = ((imm as u32) & 3) << 29;
        let immhi = (((imm as u32) >> 2) & 0x7FFFF) << 5;
        let word = immlo | (0b10000 << 24) | immhi | rd;
        return Ok(EncodeResult::Word(word));
    }
    ...
}
```

The code masks the immediate with `& 0x7FFFF` without range checking, so out-of-range values silently wrap.

## Reproduction

**Input:** `adr x0, #1048577`

**Expected:** `Err` — immediate out of range [-1048576, 1048575]

**Actual:** `Ok(Word(0x30800000))` — silently truncated

Decoding `0x30800000` gives `sign_extend(0x100001, 21)` = **`-1048575`**, so `adr x0, #1048577` silently becomes `adr x0, #-1048575` — the address loaded is off by 2097152 bytes.

## Impact

Silent mis-compilation with wrong branch target. Any ADR with `#imm` outside `[-1 MiB, +1 MiB]` emits a word targeting the wrong address. The relocation (`AdrPrelLo21`) form is unaffected — only the literal `#imm` path is broken. No diagnostic is produced, making this extremely difficult to debug.

## Suggested Fix

Validate the immediate before encoding:

```rust
if !(-(1i64 << 20)..(1i64 << 20)).contains(&imm) {
    return Err(format!(
        "adr: immediate {} out of range [-1048576, 1048575]", imm
    ));
}
```

## Regression Property

Failing property: `prop_out_of_range_immediate_rejected`

```rust
prop_assert!(encode_adr(&[xreg(0), Operand::Imm(1048577)]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/8