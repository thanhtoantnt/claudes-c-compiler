# Bug — `encode_br` silently aliases `sp`/`wsp` to `xzr`/`wzr`

**Target:** `src/backend/arm/assembler/encoder/compare_branch.rs :: encode_br`
**Severity:** Correctness / assembler-conformance (silent mis-assembly; branches to address 0)
**Pinned by:** `prop_encode_br_tests::prop_sp_silently_aliased_to_xzr`

## Summary

`encode_br` calls `get_reg` → `parse_reg_num`, which maps both `sp` and `xzr`
(and `wsp`/`wzr`) to the register number **31**. For the unconditional
branch-to-register class, Rn=31 denotes **XZR**, and there is **no SP-using
form** of `BR`. Therefore `br sp` silently produces the same word as `br xzr`
— a branch to address 0 — instead of being rejected as an unallocated encoding.

```rust
pub(crate) fn encode_br(operands: &[Operand]) -> Result<EncodeResult, String> {
    let (rn, _) = get_reg(operands, 0)?;
    let word = 0xd61f0000 | (rn << 5);     // rn == 31 for both 'sp' and 'xzr'
    Ok(EncodeResult::Word(word))
}
```

`parse_reg_num` (`encoder/mod.rs`):
```rust
"sp" | "wsp" => Some(31),
"xzr" | "wzr" => Some(31),
```

## Minimal input

```
br sp
br wsp
```

## Expected vs actual

| input | **Expected** (GAS / `llvm-mc`) | **Actual** |
|---|---|---|
| `br sp`  | `Err` — unallocated; no SP form of BR | `Ok(Word(0xd61f03e0))` == `br xzr` |
| `br wsp` | `Err` — unallocated (also wrong width, see separate report) | `Ok(Word(0xd61f03e0))` == `br wzr` |

`br xzr` is the architecturally-correct encoding of "branch to address 0";
the bug is that `br sp` is *accepted* and silently means the same thing rather
than erroring on the unintended `sp`/`wsp` spelling.

## Impact

A program intended to indirect-branch through the stack pointer is silently
recompiled into a branch to address 0, with no diagnostic. This is a serious
silent correctness loss (likely crash/hang at runtime).

## Root cause

The shared `parse_reg_num` helper is SP-aware (it folds `sp`→31), which is
correct for SP-using instructions (ADD/SUB/LDR) but wrong for every XZR-only
instruction. `BR` has no SP form and must reject `sp`/`wsp`.

## Suggested fix

In `encode_br` (and sibling `encode_blr`, `encode_ret`), reject the `sp`/
`wsp` spellings explicitly before encoding, since for these instructions
Rn=31 must mean XZR, not SP:

```rust
let name = match operands.get(0) {
    Some(Operand::Reg(n)) => n,
    _ => return Err("br requires a register".into()),
};
let lower = name.to_lowercase();
if lower == "sp" || lower == "wsp" {
    return Err("br has no SP-using form (use xzr)".into());
}
let (rn, is_64) = get_reg(operands, 0)?;
if !is_64 { return Err("br requires a 64-bit X register".into()); }
let word = 0xd61f0000 | (rn << 5);
Ok(EncodeResult::Word(word))
```

When landed, `prop_sp_silently_aliased_to_xzr`'s `is_ok()` assertions should
flip to `is_err()`.

## Related
The identical defect exists in `encode_cbz`/`encode_cbnz` (pinned by their
own `prop_sp_silently_aliased_to_xzr` test).

## Reproduce
```bash
cargo test --lib prop_sp_silently_aliased_to_xzr
```
