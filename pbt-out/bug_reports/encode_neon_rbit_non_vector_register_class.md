# Bug Report: `encode_neon_rbit` silently accepts GPR-class register names in SIMD operands

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_rbit`
**Severity:** Low

## Summary

`encode_neon_rbit` does not restrict the register operand to the SIMD register
class (V0–V31). Because the shared helper `get_neon_reg` → `parse_reg_num`
accepts any name starting with `x`/`w`/`d`/`s`/`q`/`v`/`h`/`b`, a
`RegArrangement` carrying a general-purpose register name (e.g. `x0.16b`) is
encoded as if it were `v0`. RBIT is a SIMD instruction whose operands must be
vector registers, so a GPR (or scalar FP name with the wrong class) should be
rejected, not silently reinterpreted.

This is distinct from the source-arrangement-mismatch finding
(`encode_neon_rbit_source_arrangement_mismatch.md`): that bug is about
arrangement *strings*; this one is about the register *class*.

## Root Cause

`get_neon_reg` delegates to `parse_reg_num`, which accepts all of `x`, `w`,
`d`, `s`, `q`, `v`, `h`, `b` prefixes (`encoder/mod.rs`):

```rust
'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {
    let num: u32 = name[1..].parse().ok()?;
    if num <= 31 { Some(num) } else { None }
}
```

`encode_neon_rbit` never re-checks that the resolved register is in the SIMD
class:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;
```

The parser path is reachable: `is_register` returns `true` for `x0`/`w5`/`sp`
(`parser.rs`), and `RegArrangement` construction only requires
`is_register(reg_part)`, so `x0.16b` parses to
`Operand::RegArrangement { reg: "x0", arrangement: "16b" }`.

## Reproduction

| Input                    | Actual result     | Expected |
|--------------------------|-------------------|----------|
| `rbit x0.16b, v1.16b`    | `Ok(0x6E605820)`  | `Err`    |
| `rbit w5.16b, v1.16b`    | `Ok(0x6E605820)`  | `Err`    |
| `rbit sp.16b, v1.16b`    | `Ok(0x6E605820)`  | `Err`    |

Each is encoded as if the GPR were `v0`. The `#[ignore]`d test
`rbit_rejects_non_vector_register_class` reproduces this.

## Impact

Severity is **Low** because (a) the parser only reaches this path for the
unusual `xN.<arr>` token shape, which is not what RBIT assembly normally looks
like, and (b) the silent reinterpretation uses the same numeric register index,
so the resulting instruction is "plausible." The real harm is the absence of a
diagnostic: a user who writes a GPR by mistake (e.g. confusing `rbit v0.16b,
x1`-style code) gets a binary that targets the vector register `v1` with no
error, masking the typo.

## Suggested Fix

Add a SIMD-class guard. The cleanest fix is a small predicate applied to both
operands inside `encode_neon_rbit` (or, better, a SIMD-specific variant of
`get_neon_reg` that returns `Err` for non-`v` registers):

```rust
fn is_simd_reg(name: &str) -> bool {
    name.to_lowercase().starts_with('v')
}

let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, _) = get_neon_reg(operands, 1)?;
match (operands[0], operands[1]) {
    (Operand::RegArrangement { reg: r0, .. }, Operand::RegArrangement { reg: r1, .. }) => {
        if !is_simd_reg(r0) || !is_simd_reg(r1) {
            return Err("neon rbit: operands must be SIMD registers Vd/Vn".to_string());
        }
    }
    _ => {}
}
```

(Enforcing it once in `get_neon_reg`-SIMD would also benefit all other NEON
encoders that share the helper, but that is a broader change and out of scope
for this single-function report.)

## Regression Property

Failing property: `rbit_rejects_non_vector_register_class`

```rust
#[test]
#[ignore]
fn rbit_rejects_non_vector_register_class(
    gpr in prop_oneof![Just("x0"), Just("x31"), Just("w5"), Just("sp")],
) {
    let ops = vec![
        Operand::RegArrangement { reg: gpr.to_string(), arrangement: "16b".to_string() },
        Operand::RegArrangement { reg: "v1".to_string(), arrangement: "16b".to_string() },
    ];
    prop_assert!(
        encode_neon_rbit(&ops).is_err(),
        "rbit {gpr}.16b, v1.16b must be rejected (GPR in SIMD operand); got Ok",
    );
}
```

After applying the fix, remove the `#[ignore]`.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/109
