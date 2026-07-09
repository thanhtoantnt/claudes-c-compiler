# Bug Report: `encode_neon_mvni` silently accepts invalid shift kinds (`lsr`/`asr`/`ror`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mvni`
**Severity:** Medium

## Summary

`encode_neon_mvni` accepts shift operators (`lsr`, `asr`, `ror`) that are not
valid for the `MVNI` instruction. Instead of returning `Err`, it falls through
the `else` branch of its shift matcher to `cmode = 0b0000` and emits the word
for the **no-shift** form. The offending shift is silently discarded, so the
encoded instruction has a different meaning from the source text.

The ARMv8-A ARM ("MVNI (vector)") permits **only** `LSL` and `MSL` shifts.
clang's integrated assembler rejects every other shift operator.

## Root Cause

In the `"2s" | "4s"` arm, the trailing `else` swallows any unrecognized shift
kind instead of erroring:

```rust
let cmode = if let Some(Operand::Shift { kind, amount }) = operands.get(2) {
    if kind.to_lowercase() == "lsl" {
        match *amount {
            0 => 0b0000u32, 8 => 0b0010, 16 => 0b0100, 24 => 0b0110,
            _ => return Err(format!("mvni: unsupported shift amount: {}", amount)),
        }
    } else if kind.to_lowercase() == "msl" {
        match *amount {
            8 => 0b1100u32, 16 => 0b1101,
            _ => return Err(format!("mvni: unsupported MSL shift: {}", amount)),
        }
    } else {
        0b0000   // <-- BUG: lsr/asr/ror silently become the no-shift form
    }
} else {
    0b0000
};
```

## Reproduction

Input operands (equivalent to `mvni v0.4s, #5, lsr #8`):

```rust
vec![
    Operand::RegArrangement { reg: "v0".into(), arrangement: "4s".into() },
    Operand::Imm(5),
    Operand::Shift { kind: "lsr".into(), amount: 8 },
]
```

- **Expected:** `Err(...)` — `lsr` is not a valid MVNI shift operator.
- **Actual:** `Ok(EncodeResult::Word(0x6F0004A0))` — the encoding of
  `mvni v0.4s, #5` (no shift). clang rejects the original: `error: invalid
  operand for instruction`.

## Impact

Malformed source is turned into a silently different *legal* instruction. There
is no crash and no diagnostic, so a typo (`lsr` instead of `lsl`) or a
compiler emitting a bad operand would load the wrong constant into the vector
register and produce subtly wrong code that still "works".

## Suggested Fix

Replace the fallthrough with an error:

```rust
} else {
    return Err(format!("mvni: unsupported shift kind: {} (only LSL/MSL allowed)", kind));
}
```

## Regression Property

Failing property: `mvni_rejects_invalid_shift_kind` (`#[ignore]`d)

```rust
#[test]
#[ignore]
fn mvni_rejects_invalid_shift_kind() {
    for kind in &["lsr", "asr", "ror"] {
        let ops = vec![
            Operand::RegArrangement { reg: "v0".into(), arrangement: "4s".into() },
            Operand::Imm(5),
            Operand::Shift { kind: kind.to_string(), amount: 8 },
        ];
        assert!(encode_neon_mvni(&ops).is_err(),
            "{kind} is not a valid MVNI shift; expected Err");
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/197
