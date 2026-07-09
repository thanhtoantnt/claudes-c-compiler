# BUG: `encode_ldxp_stxp` silently drops non-zero immediate offsets

## Target
`src/backend/arm/assembler/encoder/load_store.rs`, function `encode_ldxp_stxp`
(LDXP / LDAXP / STXP / STLXP — Load/Store Exclusive Pair).

## Symptom
A non-zero immediate offset on an exclusive-pair load/store is **silently
accepted and encoded as if the offset were zero** — i.e. the encoder treats
`ldxp x0,x1,[x2,#8]` identically to `ldxp x0,x1,[x2]`. No error is reported, so
an assembler user gets an instruction with the *wrong effective address* and no
diagnostic.

## Spec (ARM ARM)
The exclusive-pair load/store group has **no immediate-offset form**. The only
permitted assembler syntax is:

```
LDXP  <Xt1>, <Xt2>, [<Xn|SP>]
STXP  <Ws>, <Xt1>, <Xt2>, [<Xn|SP>]
```

Encodings (no immediate field exists in either):

```
LDXP/LDAXP: 1 sz 001000 0 1 1 11111 o0 Rt2 Rn Rt   (bit23=0, bit22=1)
STXP/STLXP: 1 sz 001000 0 0 1 Rs   o0 Rt2 Rn Rt    (bit23=0, bit22=0)
```

The `..` in the operand match below is the root cause:

```rust
let rn = match operands.get(2) {                 // (load branch)
    Some(Operand::Mem { base, .. }) => parse_reg_num(base)...,
    _ => return Err(...),
};
```
```rust
let rn = match operands.get(3) {                 // (store branch)
    Some(Operand::Mem { base, .. }) => parse_reg_num(base)...,
    _ => return Err(...),
};
```

`offset` is bound with `..` and never inspected, so any value is silently lost.

## PBT evidence
`prop_encode_ldxp_stxp_offset_tests` (new module in `load_store.rs`, 4 properties).
Two of four properties fail:

| Property | Result | Meaning |
|---|---|---|
| `prop_offset_does_not_affect_word` | **PASS** | Documents the drop: any two offsets → identical word. |
| `prop_nonzero_offset_rejected`     | **FAIL** | `off=1` returns `Ok(Word(..))`, must be `Err`. |
| `prop_zero_offset_accepted`        | **PASS** | The only legal form works. |
| `prop_common_offsets_rejected`     | **FAIL** | `off=1` returns `Ok`, must be `Err`. |

Minimal failing input (proptest):
```
is_load=false, acquire_release=false,
rt_num=0, rt2_num=0, base_num=0, ws_num=0, off=1
  → Ok(Word(3357540352))   [expected Err]
```

## Impact
Incorrect code generation for any source that writes a non-zero offset on an
exclusive pair instruction (a common mistake, e.g. `stxp w0,x1,x2,[x4,#16]`),
with no assembler diagnostic. The same defect class is already present in the
sibling `encode_ldxr_stxr` (documented by `prop_encode_ldxr_stxr_offset_tests`).

## Suggested fix
Inspect the offset in both branches of `encode_ldxp_stxp` and reject non-zero
values, e.g. for the load branch:

```rust
Some(Operand::Mem { base, offset }) => {
    if *offset != 0 {
        return Err(format!(
            "ldxp/ldaxp does not support an immediate offset (got #{}); use [Rn] only",
            offset));
    }
    parse_reg_num(base).ok_or("ldxp needs memory operand")?
}
```

(analogously for the store branch's `operands.get(3)`). After this fix the two
failing properties flip to PASS and the two passing properties continue to hold.
