# Bug Report: `encode_neon_mvni` silently drops `MSL` shift on 16-bit arrangements (`.4h`/`.8h`)

**Target:** `src/backend/arm/assembler/encoder/neon.rs` → `encode_neon_mvni`
**Severity:** Medium

## Summary

`encode_neon_mvni` ignores a shift operand entirely in its `.4h`/`.8h` branch.
An `MSL` shift (which is defined **only** for the 32-bit element form) is
therefore accepted for 16-bit elements and silently dropped: the encoder emits
the no-shift 16-bit word instead of returning `Err`.

The ARMv8-A ARM defines the 16-bit form (`cmode = 1000`) with **no shift at
all**; `MSL` is legal only for `.2s`/`.4s`. clang rejects the combination.

## Root Cause

The `"4h" | "8h"` arm builds the word with a hardcoded `cmode = 0b1000` and
never inspects `operands.get(2)`:

```rust
"4h" | "8h" => {
    let q: u32 = if arr_d == "8h" { 1 } else { 0 };
    // MVNI 16-bit: cmode=1000, op=1
    let word = (q << 30) | (1 << 29) | (0b0111100 << 22)
        | (abc << 16) | (0b1000 << 12) | (0b01 << 10) | (defgh << 5) | rd;
    Ok(EncodeResult::Word(word))   // <-- BUG: shift operand never checked
}
```

## Reproduction

Input operands (equivalent to `mvni v6.4h, #0x80, msl #8`):

```rust
vec![
    Operand::RegArrangement { reg: "v6".into(), arrangement: "4h".into() },
    Operand::Imm(0x80),
    Operand::Shift { kind: "msl".into(), amount: 8 },
]
```

- **Expected:** `Err(...)` — `MSL` is not valid for the 16-bit MVNI form.
- **Actual:** `Ok(EncodeResult::Word(0x2F048406))` — the encoding of
  `mvni v6.4h, #0x80` (no shift). clang rejects the original:
  `error: invalid operand for instruction`. Same for `.8h`.

## Impact

A misplaced `MSL` shift produces a silently different legal instruction (the
non-shifted constant) with no crash or diagnostic. The vector register receives
the wrong value and the program runs subtly wrong.

## Suggested Fix

Reject any shift operand on the 16-bit form:

```rust
"4h" | "8h" => {
    if operands.get(2).map_or(false, |o| matches!(o, Operand::Shift { .. })) {
        return Err(format!("mvni: {arr_d} form does not allow a shift"));
    }
    // ... existing encoding ...
}
```

## Regression Property

Failing property: `mvni_rejects_msl_on_16bit` (`#[ignore]`d)

```rust
#[test]
#[ignore]
fn mvni_rejects_msl_on_16bit() {
    for arr in &["4h", "8h"] {
        let ops = vec![
            Operand::RegArrangement { reg: "v6".into(), arrangement: arr.to_string().into() },
            Operand::Imm(0x80),
            Operand::Shift { kind: "msl".to_string(), amount: 8 },
        ];
        assert!(encode_neon_mvni(&ops).is_err(),
            "MSL is not valid for MVNI .{arr}; expected Err");
    }
}
```

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/197
