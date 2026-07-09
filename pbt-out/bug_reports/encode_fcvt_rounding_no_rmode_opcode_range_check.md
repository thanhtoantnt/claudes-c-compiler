# BUG: `encode_fcvt_rounding` performs no range validation on `rmode` / `opcode`

- **File:** `src/backend/arm/assembler/encoder/fp_scalar.rs`
- **Function:** `encode_fcvt_rounding(operands, rmode, opcode)`
- **Severity:** Medium (silent encoding corruption — produces architecturally
  invalid instructions instead of an error)
- **Test that exposes it:** `prop_fcvt_rounding_rejects_oversized_rmode_and_opcode`
  (FAILS). The four companion properties (field placement, sf/ftype derivation,
  determinism, out-of-range register / arity rejection) PASS.

## Summary

`encode_fcvt_rounding` ORs `rmode` and `opcode` into fixed-width instruction
fields with **no range check**:

```rust
let word = ((sf << 31) | (0b11110 << 24) | (ftype << 22)
    | (1 << 21) | (rmode << 19) | (opcode << 16)) | (rn << 5) | rd;
```

Per the ARMv8-A "Floating-point<->integer conversions" encoding
(`sf 00 11110 ftype 1 rmode opcode 000000 Rn Rd`), `rmode` is a **2-bit** field
at bits [20:19] (legal values 0–3) and `opcode` is a **3-bit** field at bits
[18:16] (legal values 0–7). Values outside these ranges are not masked or
rejected; they spill into the neighbouring fields:

- `rmode >= 4` (e.g. `rmode = 8`) sets bit 22, **flipping `ftype`** — a single-
  precision source (`ftype = 00`) is silently re-encoded as double precision
  (`ftype = 01`). It also touches the fixed bit 21 (coincidentally already `1`,
  but only by luck).
- `opcode >= 8` (e.g. `opcode = 8`) sets bit 19, **corrupting the `rmode`
  field**.

## Concrete corruption (computed)

For `w0, s0` operands (sf=0, ftype=00), `rmode=0`, `opcode=0`:

| call                                  | encoded word | ftype[23:22] | rmode[20:19] | note |
|---------------------------------------|--------------|--------------|--------------|------|
| `encode_fcvt_rounding([w0,s0], 0, 0)` | `0x1E200000` | 00 (single)  | 00           | correct |
| `encode_fcvt_rounding([w0,s0], 8, 0)` | `0x1E600000` | **01 (double!)** | 00       | single→double silently |

So passing `rmode = 8` (a 4-bit value into a 2-bit field) rewrites the source-
precision selector and emits `0x1E600000`, which the CPU decodes as a
**double-precision** conversion — a different instruction from the one the
caller asked for. No `Err` is returned.

## Impact

The function is called for the FCVT family (FCVTNS/NU/PS/PU/MS/MU/ZS/ZU/AS/AU).
The only thing keeping the fields legal today is the caller passing in-range
constants. Any caller bug, typo, or future addition that supplies an oversized
`rmode`/`opcode` produces a valid-looking but semantically wrong machine word,
with no diagnostic.

## Suggested fix

Validate the field widths before encoding and return `Err` otherwise:

```rust
if rmode > 0b11 {
    return Err(format!("fcvt*: rmode {} exceeds 2-bit field [20:19]", rmode));
}
if opcode > 0b111 {
    return Err(format!("fcvt*: opcode {} exceeds 3-bit field [18:16]", opcode));
}
```

(Optionally also assert the destination is a GP register (W/X) and the source is
an FP register (S/D), since `encode_fcvt_rounding` currently derives `sf`/`ftype`
without verifying operand banks.)
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/127
