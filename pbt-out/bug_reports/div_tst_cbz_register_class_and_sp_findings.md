# Bug Report: `encode_div` / `encode_tst` / `encode_cbz` accept FP/SIMD registers and alias SP to XZR

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs :: encode_div`,
`src/backend/arm/assembler/encoder/compare_branch.rs :: encode_tst`,
`src/backend/arm/assembler/encoder/compare_branch.rs :: encode_cbz`
**Severity:** Correctness / assembler-conformance (silent mis-assembly)
**Pinned by:** `div_tst_cbz_regclass_pbt` (6 `#[ignore]`d witnesses; run with `cargo test --lib div_tst_cbz_regclass_pbt -- --ignored`)

## Summary

All three encoders funnel operands through the shared `get_reg` → `parse_reg_num`
path. `parse_reg_num` is **register-class-agnostic** (it accepts the FP/SIMD
prefixes `d`/`s`/`q`/`v`/`h`/`b`) and maps **both** `sp`/`wsp` **and** `xzr`/`wzr`
to register number `31`. The helper `is_fp_reg` (which would catch the FP/SIMD
case) is defined in `encoder/mod.rs` but is **never called**. As a result these
GP-only instructions silently accept FP/SIMD operands (encoding them as their GP
namesakes) and silently alias `sp`/`wsp` to the zero register (encoding field 31
is XZR/WZR for all three — none has an SP form).

This extends the register-class findings already filed for `encode_br` /
`encode_blr` / `encode_ret` to the DIV / TST / CBZ class. All inputs below are
**rejected** by the system conforming assembler `clang --target=aarch64-linux-gnu`.

## Findings (new — no prior report)

| # | Function | Class | Input | `clang` | Actual encoder output |
|---|----------|-------|-------|---------|-----------------------|
| 1 | `encode_div` | FP/SIMD | `sdiv x0, v1, x2` / `sdiv d0, x1, x2` | REJECT ("invalid operand for instruction") | `Ok(Word(_))` — encoded as `sdiv x0, x1, x2` / `sdiv w0, x1, x2` |
| 2 | `encode_tst` | FP/SIMD | `tst v0, x1` / `tst x0, d1` | REJECT | `Ok(Word(_))` — encoded as the GP-namesake ANDS |
| 3 | `encode_tst` | SP aliasing | `tst sp, x0` / `tst x0, sp` | REJECT ("invalid operand for instruction") | `Ok(Word(_))` — Rn/Rm = 31 == XZR |
| 4 | `encode_cbz` | FP/SIMD | `cbz v0, lab` / `cbz d0, lab` | REJECT | `Ok(Word(_))` — encoded identically to `cbz w0, lab` |

(Findings 1–4 share the root cause below and are independent of width, which is
already reported for `encode_div` as `encode_div_silent_width_acceptance.md`.)

## Reconfirmations (already reported — covered here as regression witnesses)

| Function | Class | Prior report |
|----------|-------|--------------|
| `encode_div` | SP aliasing (`sdiv x0, sp, x1`) | `encode_div_sp_operand_silently_accepted_as_xzr.md` |
| `encode_cbz` | SP aliasing (`cbz sp, lab`) | `encode_cbz_sp_operand_silently_accepted_as_xzr.md` |

## Root Cause

`parse_reg_num` (`src/backend/arm/assembler/encoder/mod.rs`) accepts every
register prefix and cannot distinguish `sp` from `xzr`:

```rust
pub fn parse_reg_num(name: &str) -> Option<u32> {
    match name.to_lowercase().as_str() {
        "sp" | "wsp" => Some(31),
        "xzr" | "wzr" => Some(31),          // <-- indistinguishable from sp/wsp
        "lr" => Some(30),
        _ => match name.chars().next()? {
            'x' | 'w' | 'd' | 's' | 'q' | 'v' | 'h' | 'b' => { /* accepts FP/SIMD */ }
            _ => None,
        }
    }
}
```

`get_reg` calls `parse_reg_num` but **never** consults the dead `is_fp_reg`
helper. None of `encode_div`, `encode_tst`, `encode_cbz` reject FP/SIMD names or
the `sp`/`wsp` spelling.

## Reproduction

```
sdiv x0, v1, x2      # encoded as sdiv x0, x1, x2   (silently)
tst   sp, x0         # encoded as ANDS xzr, xzr, x0 (sp -> 31 == xzr)
cbz   d0, lab        # encoded as cbz w0, lab        (d0 -> 0, sf=0)
```

Run the witnesses:

```sh
cargo test --lib div_tst_cbz_regclass_pbt -- --ignored
# => 0 passed; 6 failed  (each failure is a confirmed defect)
```

The default suite stays green (the 6 properties are `#[ignore]d`):
```sh
cargo test --lib div_tst_cbz_regclass_pbt
# => 9 passed; 0 failed; 6 ignored
```

## Impact

Silent mis-assembly with no diagnostic:
- **FP/SIMD acceptance:** a floating-point/SIMD register and the GP register of
  the same number are completely unrelated. `sdiv x0, v1, x2` becomes
  `sdiv x0, x1, x2` — a correct-looking but semantically different instruction.
- **SP aliasing (TST):** ANDS encodes field 31 as XZR/WZR; `tst sp, x0` becomes
  `tst xzr, x0`, testing the wrong register.
- For `encode_div`/`encode_cbz` SP the impact is documented in their existing
  reports (a zero divisor / always-branch respectively).

## Suggested Fix

Reject FP/SIMD names and the `sp`/`wsp` spelling at the GP-operand sites. The
`is_fp_reg` helper already exists; the missing piece is rejecting SP by name
(the encoder must accept `xzr`/`wzr` but reject `sp`/`wsp`, which `parse_reg_num`
currently cannot distinguish). A minimal shared helper:

```rust
fn gp_reg(operands: &[Operand], idx: usize) -> Result<(u32, bool), String> {
    let name = match operands.get(idx) {
        Some(Operand::Reg(n)) => n,
        other => return Err(format!("expected register at operand {}, got {:?}", idx, other)),
    };
    if is_fp_reg(name) {
        return Err(format!("operand {} must be a general-purpose register, not FP/SIMD: {}", idx, name));
    }
    let low = name.to_lowercase();
    if low == "sp" || low == "wsp" {
        return Err(format!("operand {} cannot use SP/WSP (register 31 is XZR here): {}", idx, name));
    }
    get_reg(operands, idx)
}
```

Routing `encode_div`, `encode_tst`, and `encode_cbz` through `gp_reg` flips all
six `#[ignore]d` witnesses to `is_err()` (passing), at which point the
`#[ignore]` attributes can be removed.

## Regression Property

Failing properties (all in `div_tst_cbz_regclass_pbt`):

- `prop_div_rejects_fpsimd_register_class`
- `prop_div_rejects_sp_operands` (reconfirms existing report)
- `prop_tst_rejects_fpsimd_register_class`
- `prop_tst_rejects_sp_operands`
- `prop_cbz_rejects_fpsimd_register_class`
- `prop_cbz_rejects_sp_operand` (reconfirms existing report)

Minimal failing assertions:

```rust
// FP/SIMD class (new findings)
prop_assert!(encode_div(&[Operand::Reg("x0".into()), Operand::Reg("v1".into()), Operand::Reg("x2".into())], false).is_err());
prop_assert!(encode_tst(&[Operand::Reg("v0".into()), Operand::Reg("x1".into())]).is_err());
prop_assert!(encode_cbz(&[Operand::Reg("d0".into()), Operand::Symbol("lab".into())], false).is_err());

// SP aliasing
prop_assert!(encode_tst(&[Operand::Reg("sp".into()), Operand::Reg("x0".into())]).is_err());
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/332
