# Bug Report: `encode_mrs` / `encode_msr` accept FP/SIMD registers as `Rt`

**Target:** `src/backend/arm/assembler/encoder/system.rs` → `encode_mrs`, `encode_msr`
**Severity:** Medium

## Summary

The `Rt` operand of `mrs <Xt>, <sysreg>` and `msr <sysreg>, <Xt>` must be a
general-purpose (X or W) register. Both encoders route the operand through
`get_reg` → `parse_reg_num`, which happily accepts FP/SIMD register names
(`d0`, `s0`, `q5`, `v0`, …) and re-uses the lane number as `Rt`. The result is
an instruction word that is bit-for-bit identical to the GP-register spelling
(`mrs d0, sctlr_el1` encodes exactly like `mrs x0, sctlr_el1`), silently
emitting an illegal operand combination.

Canonical assemblers reject these. `clang --target=aarch64-linux-gnu`
(LLVM-MC) reports:

```
t.s:1:5: error: invalid operand for instruction
```

## Root Cause

`get_reg` matches `Operand::Reg(name)` and calls `parse_reg_num(name)`, whose
prefix dispatch accepts every register bank:

```rust
fn get_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    match operands.get(idx) {
        Some(Operand::Reg(name)) => {
            let num = parse_reg_num(name).ok_or_else(|| format!("invalid register: {}", name))?;
            ...
        }
        other => Err(...),
    }
}

pub fn parse_reg_num(name: &str) -> Option<u32> {
    ...
    match prefix {
        'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => {   // <-- FP/SIMD banks accepted
            let num: u32 = name[1..].parse().ok()?;
            if num <= 31 { Some(num) } else { None }
        }
        _ => None,
    }
}
```

`encode_mrs`/`encode_msr` never re-check that `Rt` is a GP register, so a
floating-point/SIMD lane number lands in the `Rt` field.

## Reproduction

```
mrs d0, sctlr_el1   -> encoder: Ok(0xD538_1000)   (== mrs x0, sctlr_el1)
                   -> clang:    error: invalid operand for instruction

msr sctlr_el1, q5   -> encoder: Ok(0xD518_1005)   (== msr sctlr_el1, x5)
mrs s0, midr_el1    -> encoder: Ok(0xD538_0000)   (== mrs x0, midr_el1)
```

Genuine GP-register operands (`x0..x30`, `w0..w30`, `sp`/`xzr` = 31) are
encoded correctly and match clang exactly; only the FP/SIMD spellings are
wrongly accepted.

## Impact

`mrs d0, …` / `msr …, v3` are typos a programmer can easily make, and the
assembler emits no diagnostic — the FP/SIMD register is silently reinterpreted
as a GP register number. At best this masks a source-level mistake; at worst a
computed-register macro that happens to name a vector register produces a
system-register read/write of an unintended GP register, with no object-level
signal. Silent mis-assembly.

## Suggested Fix

After `get_reg`, restrict `Rt` to the GP banks (or add a `get_gp_reg` helper):

```rust
fn is_gp_reg(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('x') || n.starts_with('w')
        || matches!(n.as_str(), "sp" | "wsp" | "xzr" | "wzr" | "lr")
}

// in encode_mrs, after reading Rt:
if let Some(Operand::Reg(name)) = operands.get(0) {
    if !is_gp_reg(name) {
        return Err(format!("mrs Rt must be a general-purpose register, got {}", name));
    }
}
```

Apply the symmetric check in `encode_msr`'s register path.

## Regression Property

Failing property: `b2_mrs_msr_reject_fp_register_rt`
(file `src/backend/arm/assembler/encoder/system_msr_mrs_pbt.rs`, marked
`#[ignore]` so the default `cargo test` stays green).

Run with: `cargo test --lib system_msr_mrs -- --ignored`

```rust
#[test]
#[ignore = "documented bug: FP/SIMD register accepted as Rt; clang rejects with 'invalid operand for instruction'"]
fn b2_mrs_msr_reject_fp_register_rt() {
    let fp_regs = ["d0", "s0", "q5", "v0"];
    for rt_name in fp_regs {
        let r = encode_mrs(&[Operand::Reg(rt_name.into()), Operand::Symbol("sctlr_el1".into())]);
        assert!(r.is_err(),
            "mrs {}, sctlr_el1 should be rejected (not a GP register), got {:?}", rt_name, r);
        let r = encode_msr(&[Operand::Symbol("sctlr_el1".into()), Operand::Reg(rt_name.into())]);
        assert!(r.is_err(),
            "msr sctlr_el1, {} should be rejected (not a GP register), got {:?}", rt_name, r);
    }
}
```

Current behaviour: the assertion fails — e.g. `mrs d0, sctlr_el1` returns
`Ok(Word(3577221120))` (`0xD538_1000`, i.e. identical to `mrs x0, sctlr_el1`).


**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/282
