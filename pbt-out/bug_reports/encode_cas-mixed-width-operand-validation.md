# `encode_cas` silently accepts register-width violations

**File:** `src/backend/arm/assembler/encoder/load_store.rs`
**Function:** `encode_cas(mnemonic: &str, operands: &[Operand]) -> Result<EncodeResult, String>`
**Status:** Bug confirmed — failing negative-contract property `prop_width_violation_rejected`
**Severity:** Medium (silent acceptance of architecturally UNDEFINED operand combinations; no wrong encoding for valid input)

## Summary

`encode_cas` derives the instruction `size` field **only** from the `Rs` register
(operand 0) and the mnemonic suffix. The width of `Rt` (operand 1) is parsed and
then **discarded**:

```rust
let (rs, is_64) = get_reg(operands, 0)?;   // is_64 used for size
let (rt, _) = get_reg(operands, 1)?;       // <-- is_64 thrown away
```

Consequently the encoder never validates the register-width constraints that the
ARMv8-A Architecture Reference Manual places on CAS (§C6.2.21 "CAS"):

1. `Rs` and `Rt` **must be the same width** (both W or both X).
2. `CASB` / `CASH` (byte / halfword forms) **must use W (32-bit) registers only**.

Both classes of invalid input are silently accepted and produce a word, so a
caller cannot distinguish a correct instruction from an UNDEFINED one. This is a
mixed-width-operand + dead-parameter defect: `Rt`'s width is computed by
`get_reg` and immediately dropped.

## Reproduction (empirically confirmed against the live encoder)

| Input                        | Result        | Expected |
|------------------------------|---------------|----------|
| `casb x0, x1, [x2]`          | `0x08A07C41`  | **Err** — byte CAS requires W regs |
| `cash x0, x1, [x2]`          | `0x48A07C41`  | **Err** — half CAS requires W regs |
| `cas  w0, x1, [x2]`          | `0x88A07C41`  | **Err** — Rs (W) and Rt (X) differ |
| `cas  x0, w1, [x2]`          | `0xC8A07C41`  | **Err** — Rs (X) and Rt (W) differ |

Note that `cas w0,x1` encodes as `size=10` (taken from `Rs=w0`) and `cas x0,w1`
encodes as `size=11` (taken from `Rs=x0`) — the `Rt` width has zero effect,
confirming it is dead. `casb x0,x1` produces the identical word to
`casb w0,w1` (`0x08A07C41`), masking an invalid register-name combination.

## Failing property

`prop_encode_cas_tests::prop_width_violation_rejected` asserts that these five
UNDEFINED operand combinations return `Err`:

- `casb`/`casab` with two X registers
- `cash` with two X registers
- `cas` with mismatched W/X `Rs`,`Rt` (both directions)

Current run:
```
prop_width_violation_rejected ... FAILED    (5 other CAS properties pass)
```

## Suggested fix

Capture and check `Rt`'s width, then enforce both constraints:

```rust
let (rs, rs_is_64) = get_reg(operands, 0)?;
let (rt, rt_is_64) = get_reg(operands, 1)?;
// word/doubleword form: Rs and Rt must agree in width
if suffix_size.is_none() && rs_is_64 != rt_is_64 {
    return Err(format!("{}: Rs and Rt must have the same width", mnemonic));
}
// byte/half variants must use 32-bit (W) registers
if suffix_size.is_some() && (rs_is_64 || rt_is_64) {
    return Err(format!("{}: byte/half CAS requires W registers", mnemonic));
}
```

## Positive coverage (still passing)

The other 5 properties — fixed bits (`[29:24]=001000`, `[23]=1`, `[21]=1`,
`[14:10]=11111`), field placement (Rs`[20:16]`, Rn`[9:5]`, Rt`[4:0]`),
acquire/release differential (only L[22]/o0[15] flip), size-from-suffix/width,
and the operand-count/non-memory/range negative contract — all pass. The goldens
they are anchored to were independently confirmed:

```
cas  x0,x1,[x2] = 0xC8A07C41   casb w0,w1,[w2] = 0x08A07C41
casa x0,x1,[x2] = 0xC8E07C41   cash w0,w1,[w2] = 0x48A07C41
casl x0,x1,[x2] = 0xC8A0FC41   cas  w0,w1,[w2] = 0x88A07C41
casal x0,x1,[x2] = 0xC8E0FC41
```

The defect above is a validation gap, not an encoding error for valid input.
