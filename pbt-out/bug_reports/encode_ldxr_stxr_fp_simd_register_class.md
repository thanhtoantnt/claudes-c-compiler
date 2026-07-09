# Bug Report: `encode_ldxr_stxr` silently accepts FP/SIMD registers

**Target:** `src/backend/arm/assembler/encoder/load_store.rs` → `encode_ldxr_stxr`
**Severity:** Medium
**Aspect:** Register-class validation

## Summary

`encode_ldxr_stxr` (LDXR/STXR/LDXRB/STXRB/LDXRH/STXRH — Load/Store Exclusive
Register, ARM ARM §C6.2.93 / §C6.2.138) only operates on **general-purpose**
registers. For the load form `LDXR <Rt>, [<Xn|SP>]` the data register `<Rt>`
must be W/X; for the store form `STXR <Ws>, <Rt>, [<Xn|SP>]` BOTH the status
register `<Ws>` and the data register `<Rt>` must be W/X. Placing an FP/SIMD
register (B/H/S/D/V/Q) in any of these positions is architecturally
**UNALLOCATED** and must be rejected with `Err`.

The encoder performs no register-class check on either operand. It calls
`get_reg(...)` for Rt (load) or for Ws and Rt (store), and the underlying
`parse_reg_num` recognises every FP/SIMD prefix (`d`/`s`/`q`/`v`/`h`/`b`,
see `mod.rs:311`); `is_fp_reg` is never consulted. As a result FP/SIMD
operands are silently accepted and — because `is_64bit_reg("<fp>")` is
`false` — encode identically to the corresponding **W** register, producing
an instruction the architecture treats as a valid GP exclusive op on a
*different* register bank with no diagnostic.

## Root Cause

```rust
pub(crate) fn encode_ldxr_stxr(operands: &[Operand], is_load: bool, forced_size: Option<u32>) -> Result<EncodeResult, String> {
    if is_load {
        let (rt, is_64) = get_reg(operands, 0)?;       // <-- accepts FP/SIMD for Rt
        ...
    } else {
        let (ws, _) = get_reg(operands, 0)?;           // <-- accepts FP/SIMD for status Ws
        let (rt, is_64) = get_reg(operands, 1)?;       // <-- accepts FP/SIMD for Rt
        ...
    }
}
```

There is no `is_fp_reg` guard on the Rt or Ws operands. This is the same
defect class already reported for the sibling branch instructions
(`encode_br_accepts_fp_simd_registers.md`,
`encode_blr-fpsimd-register-class.md`, `encode_ret-fpsimd-register-class.md`).

## Reproduction

**Minimal inputs:**

| Input | Expected | Actual |
|-------|----------|--------|
| `ldxr d0, [x0]` | `Err` | `Ok(Word(2287959040))` |
| `stxr d0, x0, [x0]` (FP status) | `Err` | `Ok(Word(3355474944))` |

Shrunk proptest counterexamples:
```
LDXR: prefix = 'd', rt_num = 0, base_num = 0            →  ldxr d0, [x0]
STXR: ws_fp = true, rt_fp = false, ws_prefix = 'd',
      ws_num = 0, rt_num = 0, base_num = 0              →  stxr d0, x0, [x0]
```

The aliasing is total: `ldxr d5/s5/q5/v5/h5/b5, [x1]` all produce the *same*
word (`2287959077`), indistinguishable from `ldxr w5, [x1]`. Likewise a store
with an FP status register silently encodes as if the status were the
matching W register.

## Impact

Silent mis-encoding with no diagnostic. An assembler user (or a programmatic
caller) who writes `ldxr d0, [x1]` or `stxr d0, x1, [x2]` expecting either an
error or a genuine FP exclusive access instead gets the encoding of the GP
form on the wrong register bank. The store case is especially insidious: an
FP status register silently aliases a GP Ws, so the exclusive-monitor status
is read from an unintended register. Severity is bounded because today's
mnemonic dispatch (`mod.rs:528-533`) never feeds FP operands to this function
from parsed text, so it is latent for the text assembler but live for any
direct/internal caller.

## Suggested Fix

Validate the register class of every GP-only operand before encoding:

```rust
fn ensure_gp(name: &str, role: &str) -> Result<(), String> {
    if is_fp_reg(name) {
        return Err(format!("ldxr/stxr {} requires a general-purpose register, got {}", role, name));
    }
    Ok(())
}
// load form:
let rt_name = match &operands[0] { Operand::Reg(r) => r.as_str(), _ => "" };
ensure_gp(rt_name, "Rt")?;
// store form:
let ws_name = match &operands[0] { Operand::Reg(r) => r.as_str(), _ => "" };
let rt_name = match &operands[1] { Operand::Reg(r) => r.as_str(), _ => "" };
ensure_gp(ws_name, "status (Ws)")?;
ensure_gp(rt_name, "Rt")?;
```

## Regression Properties

Failing witnesses (marked `#[ignore]` so the default `cargo test` stays green;
run them explicitly to reproduce):

```
cargo test --lib load_store_ldar_ldxr_class_pbt::prop_ldxr_fp_simd_register_rejected -- --ignored
cargo test --lib load_store_ldar_ldxr_class_pbt::prop_stxr_fp_simd_register_rejected -- --ignored
```

```rust
#[test]
#[ignore = "documented bug: ldxr silently accepts FP/SIMD registers (register-class)"]
fn prop_ldxr_fp_simd_register_rejected(...) {
    let res = encode_ldxr_stxr(&[Operand::Reg("d0".into()),
                                 Operand::Mem { base: "x0".into(), offset: 0 }],
                                true, None);
    prop_assert!(res.is_err());
}

#[test]
#[ignore = "documented bug: stxr silently accepts FP/SIMD status/data registers (register-class)"]
fn prop_stxr_fp_simd_register_rejected(...) {
    // stxr d0, x0, [x0]  — FP status register
    let res = encode_ldxr_stxr(&[Operand::Reg("d0".into()),
                                 Operand::Reg("x0".into()),
                                 Operand::Mem { base: "x0".into(), offset: 0 }],
                                false, None);
    prop_assert!(res.is_err());
}
```

The companion mechanism property `prop_ldxr_fp_simd_aliases_w` documents the
aliasing smoking gun and is also `#[ignore]`. Once the guards are added, all
three flip to passing and can be un-ignored.

**GitHub Issue:** (to be filed)
