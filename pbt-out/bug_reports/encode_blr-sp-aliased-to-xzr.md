# Bug — `encode_blr`: `sp` / `wsp` silently aliased to `xzr` / `wzr`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs`, function `encode_blr`

```rust
pub(crate) fn encode_blr(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let word = 0xd63f0000 | (rn << 5);
    Ok(EncodeResult::Word(word))
}
```

## The bug

The shared helper `parse_reg_num` (in `encoder/mod.rs`) maps both `sp` and `xzr` to register number `31`:

```rust
"sp" | "wsp" => Some(31),
"xzr" | "wzr" => Some(31),
```

The `BLR` instruction format reserves field value `31` for **XZR** — there is **no SP-using form** of BLR (ARM ARM C5.6.18). Therefore `blr sp` silently produces `0xD63F0000 | (31<<5)`, i.e. a branch-with-link to **address 0** (identical to `blr xzr`), instead of being rejected. `blr wsp` behaves identically vs `blr wzr`. A conforming assembler rejects both:

```
$ echo "blr sp" | llvm-mc -triple=aarch64 -show-encoding
error: invalid operand for instruction
```

## Minimal input

| Mnemonic | Encoded word | Expected |
|---|---|---|
| `blr sp` | `0xD63F0000 | (31<<5)` (== `blr xzr`) | `Err` (no SP form) |
| `blr wsp` | `0xD63F0000 | (31<<5)` (== `blr wzr`) | `Err` |

## Actual behavior (observed failure)

`encode_blr(&[Operand::Reg("sp".into())])` returns `Ok(EncodeResult::Word(0xD63F03E0))` (= `0xD63F0000 | (31<<5)`), bit-identical to `encode_blr(&[Operand::Reg("xzr".into())])`. No diagnostic; `blr sp` is silently rewritten to a branch-with-link to address 0. Confirmed by a **failing** proptest run:

```
prop_rejects_sp_wsp
  panicked: blr sp must be rejected (...), got Ok(Word(3594453984))
  minimal failing input: which = 0    (3594453984 == 0xD63F03E0)
```

## Impact

Critical silent mis-assembly: `blr sp` would branch-with-link to address 0 rather than erroring out. Combined with Finding (W-form), the encoder accepts every register-class-confused or stack-pointer misuse of BLR without warning. The identical defect affects the sibling `encode_br`, `encode_cbz`/`encode_cbnz`, and other encoders that consume `parse_reg_num`.

## Property that locks it (FAILING — bug confirmed)

`prop_encode_blr_tests::prop_rejects_sp_wsp` (in `compare_branch.rs`) is a **negative-contract** property asserting `encode_blr(sp).is_err()` and `encode_blr(wsp).is_err()`. It **FAILS** against the current implementation (`blr sp → Ok(0xD63F03E0)`). Once validation is added the property will pass; no assertion changes are needed.

## Fix

After obtaining the register name, reject SP explicitly:

```rust
let name = match operands.get(0) {
    Some(Operand::Reg(r)) => r.to_lowercase(),
    _ => return Err("blr: expected register".into()),
};
if matches!(name.as_str(), "sp" | "wsp") {
    return Err("blr: SP/WSP is not a valid operand".into());
}
```

## Regression property

Failing property: `prop_rejects_sp_wsp`

```rust
prop_assert!(encode_blr(&[Operand::Reg("sp".into())]).is_err());
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/14
