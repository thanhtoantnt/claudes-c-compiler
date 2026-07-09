# Bug: `eval_const_binop_float` ICEs on long-double operands with `0 < |v| < 1.0`

**Law:** Constant-folding a binary floating-point operation must return a value
(`Option<IrConst>`) for every *valid* finite long-double operand — it must never
panic / ICE the compiler. (Negative/result contract: `Div`/`Mod`-by-zero and
NaN may legitimately produce `inf`/`NaN`, but a plain value like `0.5L` is a
perfectly valid input that must be folded, not crash.)

**Impact:** Any C program whose compile-time constant `long double` expression
involves a value strictly between 0 and 1 in magnitude ICEs the compiler at
compile time. This range is extremely common: `1.0L + 0.5L`, `0.25L * 4.0L`,
`3.0L / 0.1L`, `0x1p-1L`, etc. all crash the build instead of producing a
value. The crash happens in the constant folder, so even `static long double x
= 0.5L + 1.0L;` is sufficient to reproduce.

**Function:** `crate::common::const_arith::eval_const_binop_float`
(the long-double path). Root cause is in the operand-construction helper
`crate::common::long_double::f64_to_f128_bytes_lossless`, called by
`IrConst::long_double(v)` — the canonical way sema/lowering build long-double
constants before delegating to `eval_const_binop_float`.

**Detected by:** Crash-only (a differential property exercising the long-double
path of `eval_const_binop_float` against native f64 add/sub panicked during
operand construction).

**Minimal input:**
```rust
let lhs = IrConst::long_double(1.0);
let rhs = IrConst::long_double(0.5);              // biased exp 1022 < 1023
let _   = eval_const_binop_float(&BinOp::Add, &lhs, &rhs);
```

**Expected:** `Some(IrConst::LongDouble(1.5, <bytes for 1.5>))`.

**Actual:** panic —
`thread '...' panicked at src/common/long_double.rs:1040:18: attempt to subtract with overflow`.

**Root cause:** In `f64_to_f128_bytes_lossless`, after handling zero/special, the
normal case computes the f128 biased exponent as:

```rust
let exp15 = (d.biased_exp as u128 - 1023 + 16383) as u128;
```

For any finite, non-zero f64 with magnitude `< 1.0` the true exponent is
negative, so `d.biased_exp < 1023` and the subtraction `biased_exp as u128 -
1023` **underflows in debug builds → panic**. This covers the whole range
`0 < |v| < 1.0` — i.e. all f64 normals with biased exponent `1..=1022` (e.g.
`0.5`, `0.1`, `1e-10`, the smallest normal `2.225e-308`) *and* every subnormal.
Only `|v| >= 1.0` and exact `0.0` avoid the panic.

(Contrast with the pre-existing report `f64_to_f128_subnormal_panic.md`, which
is a *different* function — `double_to_f128` in `src/common/encoding.rs` — that
*silently flushes subnormals to zero*. This report is a hard ICE in the
*lossless* converter in `long_double.rs`, affecting a much larger value range.)

**Severity:** High — ICE on everyday floating-point literals (`0.5L`).

**Fix direction:** compute the exponent in signed arithmetic before re-biasing,
e.g. `(d.biased_exp as i64 - 1023 + 16383) as u128`, or handle the
`biased_exp < 1023` (sub-normal-to-f128) case explicitly instead of relying on
unsigned subtraction.

**Regression test:** `src/common/const_arith_binop_prop_tests.rs` — the
deterministic test `witness_long_double_subnormal_operand_panics`, guarded with
`#[ignore]` so the default suite stays green. Reproduce against the current
SUT with:

```
cargo test --lib const_arith_binop_prop_tests::witness_long_double_subnormal_operand_panics -- --ignored
```

The two long-double *properties* (`long_double_add_sub_round_trips_to_native_f64`,
`long_double_comparison_matches_native_f64`) are temporarily restricted to the
non-crashing domain (`|v| == 0` or `|v| >= 1.0`) via `prop_assume!` so they
still provide coverage of the working range; lift that restriction once this bug
is fixed.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/310
