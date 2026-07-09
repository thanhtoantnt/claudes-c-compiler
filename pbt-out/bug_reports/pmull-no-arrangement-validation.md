# Bug Report: `encode_neon_pmull` does not validate operand arrangements

**Location:** `src/backend/arm/assembler/encoder/neon.rs`, function `encode_neon_pmull`

## Summary

`encode_neon_pmull` silently accepts operand arrangements that are invalid for the
`PMULL`/`PMULL2` mnemonics and emits an encoding that does not match the source
text, instead of rejecting them with an error.

The ARMv8-A ARM (Advanced SIMD three same, size=11 sub-space) defines only two
forms:

```
PMULL  Vd.1Q, Vn.1D, Vm.1D      // Q = 0
PMULL2 Vd.1Q, Vn.2D, Vm.2D      // Q = 1
```

Every other arrangement (`.8b`, `.16b`, `.4h`, `.8h`, `.2s`, `.4s`, `.1d`, `.2d`
applied to any operand) is UNALLOCATED. LLVM rejects them all with
`error: invalid operand for instruction` (verified with
`clang --target=aarch64 -march=armv8-a+crypto+aes`).

## Root cause

```rust
pub(crate) fn encode_neon_pmull(operands: &[Operand], is_pmull2: bool) -> Result<EncodeResult, String> {
    if operands.len() < 3 {
        return Err("pmull requires 3 operands".to_string());
    }
    let (rd, _) = get_neon_reg(operands, 0)?;   // arrangement discarded
    let (rn, _) = get_neon_reg(operands, 1)?;   // arrangement discarded
    let (rm, _) = get_neon_reg(operands, 2)?;   // arrangement discarded

    let q = if is_pmull2 { 1u32 } else { 0 };
    // ...encoding...
}
```

All three arrangement strings are discarded (`_`). `Q` is taken solely from the
`is_pmull2` flag (supplied by the caller), and the `size` field is hardcoded to
`11`. As a result any arrangement the parser can produce is accepted and emits
the same word as the canonical form.

## Impact

A source line such as `pmull v0.8b, v1.8b, v2.8b` — which is invalid AArch64
and is rejected by LLVM/GNU assemblers — is silently encoded as
`0x0EE2E020`, *identical* to the canonical `pmull v0.1q, v1.1d, v2.1d`. The
generated object will disassemble as a valid PMULL, masking a user-level error
in the source and producing a program whose machine code does not match its
assembly text. This is the same class of bug as the documented PMUL
non-byte-arrangement issue.

## Reproduction

Failing, shrunk `proptest!` property `rejects_non_canonical_arrangements`:

```text
$ cargo test --lib neon_pmull_pbt::rejects_non_canonical_arrangements

minimal failing input: arr = "8b", is_pmull2 = false

thread '...rejects_non_canonical_arrangements' panicked:
  Test failed: pmull with .8b on all operands is UNALLOCATED
  (only .1q/.1d or .1q/.2d); expected Err, got Ok(Word(249749536))
```

`Ok(Word(249749536))` = `0x0EE2E020`, i.e. the canonical
`pmull v0.1q, v1.1d, v2.1d` word — proving the `.8b` operands were ignored.
Counterexample persisted by proptest at
`proptest-regressions/backend/arm/assembler/encoder/neon_pmull_pbt.txt`.

## Suggested fix

Validate arrangements before encoding:

```rust
let (rd, arr_d) = get_neon_reg(operands, 0)?;
let (rn, arr_n) = get_neon_reg(operands, 1)?;
let (rm, arr_m) = get_neon_reg(operands, 2)?;

if arr_d != "1q" {
    return Err(format!("pmull: destination must be .1q, got .{}", arr_d));
}
let want = if is_pmull2 { "2d" } else { "1d" };
for (a, i) in [(arr_n.as_str(), 1), (arr_m.as_str(), 2)] {
    if a != want {
        return Err(format!(
            "pmull{}: source operand {} must be .{}, got .{}",
            if is_pmull2 { "2" } else { "" }, i, want, a,
        ));
    }
}
```

(Also note: the destination is always `.1Q` for both forms; only the sources
differ. If the codebase ever wants to infer `is_pmull2` from `.1d` vs `.2d`
sources instead of passing a flag, that can be done here.)

## Verification

- The 5 passing properties in `neon_pmull_pbt.rs` (golden-table,
  reference-encoder differential, field round-trip + Q mapping, fixed-bits
  invariant, operand-count rejection) confirm the encoding is *correct* for all
  canonical inputs — the bug is purely missing input validation, not a
  mis-encoding.
- Golden words were cross-checked against LLVM
  (`clang --target=aarch64 -march=armv8-a+crypto+aes`).
