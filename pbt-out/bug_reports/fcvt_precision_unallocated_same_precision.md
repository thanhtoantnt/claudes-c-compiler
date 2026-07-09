# Bug: `encode_fcvt_precision` emits UNALLOCATED encodings for same-precision conversions

## Location
`src/backend/arm/assembler/encoder/fp_scalar.rs`, function `encode_fcvt_precision`.

## Summary
FCVT converts between *different* floating-point precisions (e.g. `FCVT Dd, Sn`,
`FCVT Sd, Dn`, `FCVT Hd, Sn`). The encoder derives `ftype` (source precision)
and `opc` (destination precision) independently from the two operand prefixes
but never checks that they differ. When source and destination precisions match
(`Sd,Sn`, `Dd,Dn`, `Hd,Hn`), it emits `ftype == opc`, which is an
**UNALLOCATED** encoding in ARMv8-A — the ARM ARM defines no FCVT variant that
keeps the precision. The function instead returns `Ok(Word(...))`, producing a
word that is either unallocated or aliases a *different* instruction.

## Severity
**High.** A silent success for an illegal instruction means downstream
consumers (assemblers, JITs, disassemblers) receive an incoherent word with no
diagnostic. For example `FCVT S0, S0` yields `0x1E224000`; `FCVT D0, D0` yields
`0x1E604000` which is the exact encoding of **FMOV Dd, Dn** — i.e. the
assembler silently turns a bogus `FCVT` into a real, unrelated move instruction.

## ARMv8-A reference
Floating-point data-processing (1 source), `0 00 11110 ftype 1 0001 opc 10000 Rn Rd`.
The only allocated FCVT forms require source precision != dest precision:
```
FCVT <Sd>, <Dn>     FCVT <Sd>, <Hn>
FCVT <Dd>, <Sn>     FCVT <Dd>, <Hn>
FCVT <Hd>, <Sn>     FCVT <Hd>, <Dn>
```
There is **no** `FCVT Sd,Sn`, `FCVT Dd,Dn`, or `FCVT Hd,Hn`; the combinations
where `ftype == opc` are UNALLOCATED.

## Reproduction (property test)
```
cargo test --lib prop_fcvt_precision_rejects_same_precision
```
Minimal failing input: `kind = 0, n = 0` → operands `FCVT s0, s0`
→ `Ok(Word(0x1E224000))` (505561088), expected `Err`.

## PBT results (5 properties added to `fp_scalar.rs`)
| Property | Result |
|---|---|
| `prop_fcvt_precision_places_fields` (field layout, distinct precision) | PASS |
| `prop_fcvt_precision_ftype_and_opc_derivation` | PASS |
| `prop_fcvt_precision_is_deterministic` | PASS |
| `prop_fcvt_precision_rejects_bad_operands` (range/arity/bank) | PASS |
| `prop_fcvt_precision_rejects_same_precision` | **FAIL** (this bug) |

## Suggested fix
After computing `ftype` and `opc`, reject equal precisions before assembling the
word:

```rust
if ftype == opc {
    return Err(format!(
        "fcvt: source and dest precision must differ (both {:?})",
        dst_name.chars().next().unwrap()
    ));
}
```
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/132
