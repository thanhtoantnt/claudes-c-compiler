# Bug Report: `encode_neon_mvni` silently truncates out-of-range immediate (`& 0xFF`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mvni`
**Severity:** Medium

## Summary

`encode_neon_mvni` masks the immediate with `imm as u32 & 0xFF` instead of
validating that it lies in the architectural range `[0, 255]`. An immediate
outside that range (e.g. `#256`, `#0x1ff`) or a negative immediate (`#-1`) is
silently truncated to a valid 8-bit value and a `Word` is returned instead of
`Err`. clang rejects all such inputs with `error: immediate must be an integer
in range [0, 255]`.

There is no docstring, spec note, or existing test documenting this masking as
intentional, so per the negative-contract rule it is treated as a bug.

## Root Cause

```rust
let imm = get_imm(operands, 1)?;
let imm8 = imm as u32 & 0xFF;   // <-- BUG: silent truncation, no range check
```

`get_imm` returns the raw `i64`. The cast `imm as u32` then `& 0xFF` maps:
`256 -> 0x00`, `0x1ff -> 0xFF`, and `-1 -> 0xFF` (two's-complement wrap then
mask).

## Reproduction

Input operands (equivalent to `mvni v0.4s, #256`):

```rust
vec![
    Operand::RegArrangement { reg: "v0".into(), arrangement: "4s".into() },
    Operand::Imm(256),
]
```

- **Expected:** `Err(...)` — clang: `immediate must be an integer in range [0, 255]`.
- **Actual:** `Ok(EncodeResult::Word(0x6F000400))` — the encoding of
  `mvni v0.4s, #0` (256 masked to 0). Likewise `-1` masks to `0xFF`
  (`0x6F0707E0`), and `#0x1ff` masks to `0xFF`.

## Impact

An out-of-range immediate (typo, or a compiler emitting a constant > 255)
produces a silently different legal instruction — the masked value — with no
crash or diagnostic. The vector register receives the wrong constant and the
program runs subtly wrong.

## Suggested Fix

Validate the range before masking:

```rust
let imm = get_imm(operands, 1)?;
if !(0..=255).contains(&imm) {
    return Err(format!("mvni: immediate must be in range [0, 255], got {imm}"));
}
let imm8 = imm as u32 & 0xFF;
```

## Regression Property

Failing properties: `mvni_rejects_out_of_range_immediate`,
`mvni_rejects_negative_immediate` (both `#[ignore]`d)

```rust
#[test]
#[ignore]
fn mvni_rejects_out_of_range_immediate() {
    let ops = vec![
        Operand::RegArrangement { reg: "v0".into(), arrangement: "4s".into() },
        Operand::Imm(256),
    ];
    assert!(encode_neon_mvni(&ops).is_err(),
        "MVNI immediate 256 is out of range [0,255]; expected Err");
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/195
