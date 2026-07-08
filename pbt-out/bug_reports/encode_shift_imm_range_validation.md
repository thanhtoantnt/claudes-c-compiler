# Bug: `encode_shift` panics / silently mis-encodes out-of-range immediate shifts

**File:** `src/backend/arm/assembler/encoder/data_processing.rs`
**Function:** `encode_shift` (immediate form, lines 796–837)
**Severity:** High — assembler crash (DoS of the compiler) on crafted input; silent
generation of invalid AArch64 machine code on other shift kinds / release builds.

## Summary

`encode_shift`'s immediate form accepts **any** `Operand::Imm` value without range
validation, then derives the BFM/EXTR `immr`/`imms` fields via unsigned subtraction.
For an out-of-range or negative immediate this either:

1. **Panics** (debug builds) — `width - imm` / `width - 1 - imm` underflows for the
   `LSL` branch, aborting the whole compiler process; or
2. **Silently emits a garbage word** (release builds, and the `LSR`/`ASR`/`ROR`
   branches in all builds) — the immediate is truncated by `*imm as u32` and placed
   into the 6-bit fields without any check, producing an UNDEFINED encoding.

The ARMv8 ARM defines finite legal ranges for these aliases:
`LSL #imm` ∈ [0, width−1], `LSR`/`ASR` ∈ [1, width], `ROR` ∈ [1, width−1]
(`width` = 32 for W, 64 for X). GAS and LLVM reject anything outside these ranges.

## Reproduction

Property `shift_immediate_rejects_out_of_range` (added to `data_processing.rs`'s test
module) fails with a panic. Minimal counterexample reported by proptest:

```
minimal failing input: rd = 0, rn = 0, st = 0, is_64 = false, over = 1, neg = false
```

i.e. the assembly input **`lsl w0, w0, #33`** reaches:

```rust
// data_processing.rs, LSL branch (shift_type == 0b00)
let imm = *imm as u32;          // 33
let width = 32u32;              // 32-bit (W) register
let immr = (width - imm) % width;   // 32u32 - 33u32  -> PANIC: attempt to subtract with overflow
let imms = width - 1 - imm;          // (would also panic)
```

Direct test output:
```
thread '...shift_immediate_rejects_out_of_range' panicked at data_processing.rs:810:28:
attempt to subtract with overflow
```

Reachability: the encoder is wired straight from the mnemonic dispatcher
(`src/backend/arm/assembler/encoder/mod.rs:287` — `"lsl" => encode_shift(operands, 0b00)`),
so any source / inline-asm producing `lsl Wd, Wn, #N` with `N ≥ 32` (or `≥ 64` for X)
crashes the compiler. A negative immediate (`lsl w0, w0, #-1`) is even easier: `as u32`
turns it into `0xFFFFFFFF`, again underflowing the subtraction.

## Impact

- **Crash:** a single malformed shift-immediate operand aborts compilation (debug /
  checked builds, and any build that leaves overflow checks on). This is a
  denial-of-service vector for any toolchain front-end consuming untrusted assembly.
- **Mis-compilation:** in `LSR`/`ASR`/`ROR` branches (and release-mode `LSL`), no
  subtraction guards the path, so the bogus value is packed straight into `immr`/
  `imms` and emitted as a *valid-looking but semantically wrong* 32-bit instruction.
  The assembler reports success on an UNDEFINED encoding.

## Suggested fix

Validate the immediate against the per-kind legal range *before* the field math, and
return `Err` on violation — mirroring the negative contract already enforced elsewhere
in this file (e.g. `encode_add_sub`'s `unencodable_immediate_returns_err`, and the
MOVZ/MOVK/MOVN `rejects_out_of_range_immediate` properties):

```rust
if let Some(Operand::Imm(imm_val)) = operands.get(2) {
    let width = if is_64 { 64 } else { 32 };
    let lo = if shift_type == 0b00 { 0i64 } else { 1 };   // LSL permits 0; others >= 1
    let hi = match shift_type {
        0b00 | 0b11 => width as i64 - 1,   // LSL / ROR: .. width-1
        _             => width as i64,     // LSR / ASR: .. width
    };
    if *imm_val < lo || *imm_val > hi {
        return Err(format!(
            "shift immediate #{} out of range [{}, {}] for {}-bit register",
            imm_val, lo, hi, width
        ));
    }
    // ... existing field computation, now guaranteed in range ...
}
```

## Test coverage added

Four `proptest!` properties in `data_processing.rs` (`mod tests`):

| # | Property | Result |
|---|----------|--------|
| 1 | `shift_immediate_bfm_field_placement` — LSL/LSR/ASR UBFM/SBFM fields for in-range imm | ✅ pass |
| 2 | `shift_immediate_ror_extr_field_placement` — ROR EXTR fields for in-range imm | ✅ pass |
| 3 | `shift_register_form_field_placement` — data-proc(2-source) fields, all 4 kinds | ✅ pass |
| 4 | `shift_immediate_rejects_out_of_range` — out-of-range/negative imm must be `Err` | ❌ **fails (panic)** |

Properties 1–3 confirm the encoder's field placement is otherwise spec-correct; the
defect is isolated to the missing input-range validation on the immediate path.
**GitHub Issue:** https://github.com/thanhtoantnt/claudes-c-compiler/issues/93
