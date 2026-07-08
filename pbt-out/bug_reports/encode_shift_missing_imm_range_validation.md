# Bug — `encode_shift` immediate form lacks shift-amount range validation

**Target:** `src/backend/arm/assembler/encoder/data_processing.rs` → `encode_shift`

## One-sentence summary
The immediate-shift branch of `encode_shift` never validates the `#imm` amount, so an
out-of-range amount (e.g. `lsl w0, w1, #32`) **panics in debug builds** via integer
underflow instead of returning `Err` — the same root cause also lets invalid amounts for
LSR/ASR/ROR and negative amounts be silently mis-encoded as `Ok`.

## Legal range (AArch64 ARM), for width = 32 (W) or 64 (X)
LSL: `0..=width-1` · LSR/ASR/ROR: `1..=width-1`. GAS and `llvm-mc` reject anything else.

## Minimal failing input (from the property, after shrinking)
```
st = 0 (LSL), is_64 = false  →  width = 32,  imm = 32   (exactly the boundary)
operands = [ w0, w1, Imm(32) ]
```
```
panicked at data_processing.rs:810:28: attempt to subtract with overflow  (also :811:28)
property verdict: "out-of-range imm=32 (width=32, st=0) PANICKED instead of returning Err"
```

## Expected vs actual
- **Expected:** `Err("shift amount 32 out of range …")`.
- **Actual (LSL, imm ≥ width):** debug panic from `width - 1 - imm` (line 811) /
  `(width - imm) % width` (line 810) underflow; silent wraparound in release.

## Root cause (lines 803–837)
```rust
if let Some(Operand::Imm(imm)) = operands.get(2) {
    let imm = *imm as u32;                 // no negativity / range check
    let width = if is_64 { 64 } else { 32 };
    0b00 => { let immr = (width - imm) % width;   // underflow if imm > width
              let imms = width - 1 - imm;          // underflow if imm >= width
              ... }
    0b01 / 0b10 / 0b11 => { ... imm used unchecked ... }   // no guard either
}
```
There is no `if imm >= width { return Err(...) }` and no `if *imm < 0` check.

## Impact
- Emits a word the CPU treats as UNDEFINED, or a different instruction; assembled
  output is not equivalent to source.
- A malformed `.s` (or typo like `lsl x0, x1, #64`) **crashes the assembler** in debug
  instead of reporting a clean error.
- Release builds silently mis-encode — worse than crashing.

## Suggested fix
Add the guard right after coercing the immediate, before any arithmetic:
```rust
let imm_i = *imm as i64;
if imm_i < 0 { return Err(format!("shift amount must be non-negative: {}", imm_i)); }
let imm = imm_i as u32;
let width = if is_64 { 64 } else { 32 };
let lo = if shift_type == 0b00 { 0u32 } else { 1u32 }; // LSR/ASR/ROR need >= 1
if !(lo..width).contains(&imm) {
    return Err(format!("shift amount {} out of range [{}, {}] for {}-bit register",
                       imm, lo, width - 1, width));
}
```

## Test status (encode_shift)
| Property | Result |
|----------|--------|
| `shift_immediate_full_word_matches_arm_reference` (LSL/LSR/ASR differential) | ✅ pass |
| `shift_ror_immediate_full_word_matches_arm_reference` (EXTR differential)     | ✅ pass |
| `shift_register_full_word_matches_arm_reference` (2-source differential)     | ✅ pass |
| `shift_immediate_in_range_is_ok_and_fields_bounded` (positive contract)      | ✅ pass |
| `shift_immediate_out_of_range_never_returns_ok` (negative contract)          | ❌ fail — bug |

The differential oracles confirm the *encoding formula is correct for all legal inputs*;
the only defect is the missing range-validation guard.
