# Bug: `encode_add_sub` immediate path silently truncates an oversized `lsl #12` value (`& 0xFFF`)

**Function:** `encode_add_sub` (immediate path) — `src/backend/arm/assembler/encoder/data_processing.rs`
**Detected by:** Property-based test — negative/error contract (differential vs `clang --target=aarch64`)
**Severity:** Medium (assembles an instruction the programmer did not write, with no diagnostic)

## Law

The ARMv8 ARM *Add/subtract (immediate)* form with `sh = 1` places the immediate in the
12-bit `imm12` field, so the operand of an explicit `lsl #12` must be in the range
`0..=4095` (`0x000..=0xFFF`). An out-of-range value has no valid encoding and must be
rejected at assembly time. `clang --target=aarch64-linux-gnu` rejects it:

```text
$ echo '.text
add x0, x0, #0x1000, lsl #12' | clang --target=aarch64-linux-gnu -c -x assembler -
error: expected compatible register, symbol or integer in range [0, 4095]
```

## Root cause

In the immediate branch, the explicit-`lsl #12` arm masks the value instead of range-checking it:

```rust
let (imm12, sh) = if explicit_shift {
    // Explicit lsl #12: use the immediate as-is (must fit in 12 bits)
    ((imm_val as u32) & 0xFFF, 1u32)          // <-- FINDING: masks, never rejects
} else if imm_val <= 0xFFF {
    (imm_val as u32, 0u32)
} ...
```

The `& 0xFFF` silently drops any high bits, so `#0x1000` becomes `imm12 = 0`, `#0x1001`
becomes `imm12 = 1`, etc. The comment "must fit in 12 bits" documents the contract that
the code does not enforce.

## Minimal input / reproducer

```
witness property:  addsub_imm_lsl12_rejects_oversized_immediate
minimal failing input: n = 0, k = 4096, is_sub = false
```

Source: `add x0, x0, #0x1000, lsl #12`

- **Expected:** `Err` — immediate `0x1000` exceeds the 12-bit `imm12` field for the
  explicit-`lsl #12` form (valid range `0..=0xFFF`).
- **Actual:** `Ok(Word(0x91400000))` — silently encoded as `add x0, x0, #0, lsl #12`
  (`imm12 = 0`, `sh = 1`), i.e. a *different* instruction from the one requested.

All values in `0x1000..=0x7FFF` are silently truncated; for `k` with `(k & 0xFFF) != 0`
the emitted immediate is wrong, for `k = 0x1000` it collapses to zero.

## Impact

Silent mis-assembly: a source line `add x0, x0, #0x1000, lsl #12` (which clang rejects)
is accepted and emits a valid-but-wrong instruction. Downstream consumers (assembler
users, the linker, tests) cannot detect the corruption from the emitted word — it looks
like a legitimate `add x0, x0, #0, lsl #12`.

## Suggested fix

Range-check instead of masking:

```rust
let (imm12, sh) = if explicit_shift {
    if imm_val > 0xFFF {
        return Err(format!(
            "add/sub immediate {} does not fit in imm12 with lsl #12 (max 0xFFF)", imm_val));
    }
    (imm_val as u32, 1u32)
} else ...
```

## Regression property

Failing witness (marked `#[ignore]` so the default suite stays green):

```rust
// in src/backend/arm/assembler/encoder/data_processing_addsub_div_bitmask_pbt.rs
#[ignore = "documented bug: add/sub immediate lsl #12 silently truncates (>0xFFF) instead of erroring (clang rejects)"]
#[test]
fn addsub_imm_lsl12_rejects_oversized_immediate(n in 0u32..=30, k in 0x1000u32..=0x7FFF, is_sub in any::<bool>()) {
    let ops = vec![xreg(n), xreg(n), Operand::Imm(k as i64),
                   Operand::Shift { kind: "lsl".into(), amount: 12 }];
    prop_assert!(encode_add_sub(&ops, is_sub, false).is_err());
}
```

Run the witness:

```text
cargo test --lib data_processing_addsub_div_bitmask_pbt::addsub_imm_lsl12_rejects_oversized_immediate -- --ignored
```

## Related reports

This is the same root-cause site surfaced through different entry points:

- [`encode_cmn_lsl12_immediate_silent_truncation.md`](encode_cmn_lsl12_immediate_silent_truncation.md) — symptom at `CMN`
- [`encode_cmp_lsl12_immediate_silent_truncation.md`](encode_cmp_lsl12_immediate_silent_truncation.md) — symptom at `CMP`

This report is filed for the **affected function itself** (`encode_add_sub`), per
one-report-per-affected-function.

**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/263
